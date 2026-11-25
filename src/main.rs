//! OpenIPA Compiler — консольная точка входа.

use anyhow::{bail, Context, Result};
use clap::Parser;
use openipa::assets;
use openipa::build::{self, BuildCache};
use openipa::check::{self, CheckOptions};
use openipa::cli::{exit, Cli, Command, Configuration, PackageCommand, SdkCommand, TemplateKind};
use openipa::compiler::{ClangDriver, CompilerError, SwiftCapability, SwiftDriver};
use openipa::config::{Config, OutputFormat};
use openipa::diagnostics::{
    self, compiler as compiler_diag, Diagnostic, DiagnosticBundle, Severity,
};
use openipa::linker::Linker;
use openipa::packager::{self, default_info_plist, BundleOptions};
use openipa::project::info_plist;
use openipa::project::{Target, XcodeProject};
use openipa::sdk::SdkReport;
use openipa::sdk::{self, SdkRegistry};
use openipa::toolchain::Toolchain;
use openipa::validator;
use openipa::VERSION;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

fn color_enabled(cli: &Cli) -> bool {
    !cli.no_color && !cli.json && std::env::var("NO_COLOR").is_err()
}

fn paint(text: &str, code: &str, enabled: bool) -> String {
    if enabled {
        format!("\x1b[{code}m{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

fn status(msg: &str, enabled: bool) {
    eprintln!("{} {}", paint("[OpenIPA]", "1;36", enabled), msg);
}

fn success(msg: &str, enabled: bool) {
    eprintln!("{}", paint(&format!("SUCCESS: {msg}"), "1;32", enabled));
}

fn fail(msg: &str, enabled: bool) {
    eprintln!("{}", paint(&format!("ERROR: {msg}"), "1;31", enabled));
}

fn print_diagnostics(diags: &[Diagnostic], cfg: &Config, colored: bool) {
    if cfg.output_format == OutputFormat::Json {
        let bundle = DiagnosticBundle::new(diags.to_vec());
        println!("{}", bundle.to_json_pretty().unwrap());
        return;
    }
    eprint!("{}", diagnostics::render(diags, colored));
}

fn print_compiler_output(raw: &str, cfg: &Config, colored: bool, verbose: bool) {
    let diags = compiler_diag::parse(raw);
    print_diagnostics(&diags, cfg, colored);
    if (verbose || diags.is_empty()) && !raw.trim().is_empty() {
        eprintln!("--- original compiler output ---");
        eprint!("{raw}");
        if !raw.ends_with('\n') {
            eprintln!();
        }
    }
}

fn compiler_error_message(
    error: &anyhow::Error,
    cfg: &Config,
    colored: bool,
    verbose: bool,
) -> String {
    if let Some(ce) = error.downcast_ref::<CompilerError>() {
        print_compiler_output(&ce.stderr, cfg, colored, verbose);
        ce.message.clone()
    } else {
        error.to_string()
    }
}

fn main() {
    let cli = Cli::parse();
    let code = run(&cli);
    std::process::exit(code);
}

fn run(cli: &Cli) -> i32 {
    let colored = color_enabled(cli);
    let mut config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            fail(&format!("конфигурация: {e}"), colored);
            return exit::USAGE;
        }
    };
    config.verbose = cli.verbose;
    config.color = colored;
    if cli.json {
        config.output_format = OutputFormat::Json;
    }
    if let Err(e) = config.ensure_dirs() {
        fail(&format!("каталоги: {e}"), colored);
        return exit::USAGE;
    }

    let result = match &cli.command {
        Command::Init {
            name,
            template,
            output,
        } => cmd_init(cli, &config, name, *template, output.as_deref()),
        Command::Build {
            project,
            configuration,
            target,
            sdk,
            clean,
            jobs,
        } => cmd_build(
            cli,
            &config,
            project,
            *configuration,
            target.as_deref(),
            sdk,
            *clean,
            *jobs,
        ),
        Command::Check {
            project,
            configuration,
            target,
            strict,
        } => cmd_check(
            cli,
            &config,
            project,
            *configuration,
            target.as_deref(),
            *strict,
        ),
        Command::Explain { code } => cmd_explain(cli, &config, code),
        Command::Inspect { ipa } => cmd_inspect(cli, &config, ipa),
        Command::Validate { ipa } => cmd_validate(cli, &config, ipa),
        Command::Clean { project } => cmd_clean(cli, &config, project),
        Command::Doctor => cmd_doctor(cli, &config),
        Command::Version => {
            println!("OpenIPA Compiler {VERSION}");
            Ok(())
        }
        Command::Sdk { action } => cmd_sdk(cli, &config, action),
        Command::Package { action } => cmd_package(cli, &config, action),
    };

    match result {
        Ok(()) => exit::OK,
        Err(e) => {
            fail(&format!("{e:#}"), colored);
            exit::FAILURE
        }
    }
}

// ------------------------------------------------------------------ check

struct Prepared {
    project: XcodeProject,
    target: Target,
    configuration: String,
    project_root: PathBuf,
    sdk_report: Option<SdkReport>,
    sdk_path: Option<PathBuf>,
    toolchain: Toolchain,
    swift_capability: Option<SwiftCapability>,
}

fn prepare(
    cli: &Cli,
    config: &Config,
    project_path: &Path,
    configuration: Configuration,
    target_name: Option<&str>,
) -> Result<Prepared> {
    let project = openipa::project::load_project(project_path)?;
    let target = if let Some(name) = target_name {
        project
            .targets
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| anyhow::anyhow!("target '{name}' не найден"))?
            .clone()
    } else {
        project
            .application_targets()
            .first()
            .map(|t| (*t).clone())
            .ok_or_else(|| anyhow::anyhow!("в проекте нет application target"))?
    };
    let toolchain = Toolchain::detect()?;
    let registry = SdkRegistry::load(config)?;
    let sdk_entry = registry.resolve("", config).ok();
    let sdk_path = sdk_entry.map(|e| e.path);
    let sdk_report = match &sdk_path {
        Some(path) => Some(sdk::inspect_sdk(path)?),
        None => None,
    };
    let swift_capability = if !target.swift_sources().is_empty() {
        Some(SwiftDriver::new(&toolchain, sdk_report.clone()).capability())
    } else {
        None
    };
    let _ = cli;
    Ok(Prepared {
        project,
        target,
        configuration: configuration.to_string(),
        project_root: project_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(".")),
        sdk_report,
        sdk_path,
        toolchain,
        swift_capability,
    })
}

fn run_checks(prepared: &Prepared) -> Vec<Diagnostic> {
    check::analyze(&CheckOptions {
        project: &prepared.project,
        target: &prepared.target,
        configuration: &prepared.configuration,
        project_root: &prepared.project_root,
        sdk_report: prepared.sdk_report.as_ref(),
        toolchain: &prepared.toolchain,
        swift_capability: prepared.swift_capability.as_ref(),
    })
}

fn cmd_check(
    cli: &Cli,
    config: &Config,
    project_path: &Path,
    configuration: Configuration,
    target_name: Option<&str>,
    strict: bool,
) -> Result<()> {
    let colored = color_enabled(cli);
    status("Проверка проекта (до компиляции)...", colored);
    let prepared = match prepare(cli, config, project_path, configuration, target_name) {
        Ok(p) => p,
        Err(e) => {
            let mut d = Diagnostic::error("E0001", e.to_string()).with_file(project_path);
            if let Some(catalog) = diagnostics::catalog::lookup("E0001") {
                d = d.with_hint(catalog.help);
            }
            print_diagnostics(&[d], config, colored);
            bail!("проект не может быть проверен");
        }
    };
    let mut diagnostics = run_checks(&prepared);
    diagnostics::apply_strict(&mut diagnostics, strict);
    print_diagnostics(&diagnostics, config, colored);
    if diagnostics::has_errors(&diagnostics) {
        bail!(
            "найдено ошибок: {}",
            diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count()
        );
    }
    if config.output_format != OutputFormat::Json {
        success("ошибок не найдено", colored);
    }
    Ok(())
}

fn cmd_explain(cli: &Cli, _config: &Config, code: &str) -> Result<()> {
    let colored = color_enabled(cli);
    match diagnostics::catalog::lookup(code) {
        Some(entry) => {
            println!(
                "{} {}",
                paint(&format!("[{}]", entry.code), "1;36", colored),
                paint(entry.title, "1", colored)
            );
            println!("\n{}", entry.explanation);
            println!("\nhelp: {}", entry.help);
            if !entry.aliases.is_empty() {
                println!("aliases: {}", entry.aliases.join(", "));
            }
            Ok(())
        }
        None => bail!("неизвестный код диагностики: {code}"),
    }
}

// ------------------------------------------------------------------ init

fn cmd_init(
    cli: &Cli,
    _config: &Config,
    name: &str,
    template: TemplateKind,
    output: Option<&Path>,
) -> Result<()> {
    let colored = color_enabled(cli);
    let base = output
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let root = base.join(name);
    if root.exists() {
        bail!("каталог {} уже существует", root.display());
    }
    let app = root.join(name);
    let xcodeproj = root.join(format!("{name}.xcodeproj"));
    fs::create_dir_all(&app)?;
    fs::create_dir_all(&xcodeproj)?;
    fs::create_dir_all(app.join("Assets.xcassets/AppIcon.appiconset"))?;
    fs::create_dir_all(app.join("Resources"))?;

    let (sources, launch): (Vec<(String, String)>, &str) = match template {
        TemplateKind::Swiftui => (
            vec![
                (
                    format!("{name}App.swift"),
                    format!(
                        "import SwiftUI\n\n@main\nstruct {name}App: App {{\n    var body: some Scene {{\n        WindowGroup {{\n            ContentView()\n        }}\n    }}\n}}\n"
                    ),
                ),
                (
                    "ContentView.swift".into(),
                    "import SwiftUI\n\nstruct ContentView: View {\n    var body: some View {\n        Text(\"Hello, OpenIPA\")\n            .padding()\n    }\n}\n".into(),
                ),
            ],
            "SwiftUI",
        ),
        TemplateKind::Uikit => (
            vec![(
                format!("{name}App.swift"),
                "import UIKit\n\n@main\nclass AppDelegate: UIResponder, UIApplicationDelegate {\n    var window: UIWindow?\n    func application(_ application: UIApplication,\n        didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?) -> Bool {\n        let window = UIWindow(frame: UIScreen.main.bounds)\n        window.rootViewController = UIViewController()\n        window.makeKeyAndVisible()\n        self.window = window\n        return true\n    }\n}\n"
                    .to_string(),
            )],
            "UIKit",
        ),
        TemplateKind::C => (
            vec![(
                "main.c".into(),
                "int main(void) {\n    return 0;\n}\n".into(),
            )],
            "C",
        ),
    };
    for (file, body) in &sources {
        fs::write(app.join(file), body)?;
    }
    let plist_xml = info_plist::to_xml_string(&default_info_plist(
        name,
        &format!("com.example.{name}"),
        "16.0",
        name,
    ));
    fs::write(app.join("Info.plist"), plist_xml)?;
    fs::write(
        app.join("Assets.xcassets/Contents.json"),
        "{\"info\":{\"author\":\"xcode\",\"version\":1}}\n",
    )?;
    fs::write(
        app.join("Assets.xcassets/AppIcon.appiconset/Contents.json"),
        "{\"images\":[],\"info\":{\"author\":\"xcode\",\"version\":1}}\n",
    )?;

    let pbx = generate_pbxproj(name, &sources);
    fs::write(xcodeproj.join("project.pbxproj"), pbx)?;
    fs::write(
        root.join("README.md"),
        format!("# {name}\n\nСоздано `openipa init` (шаблон {launch}).\n\n```sh\nopenipa build {name}.xcodeproj\n```\n"),
    )?;

    status(&format!("создан проект {}", root.display()), colored);
    success(&format!("каталог {}", root.display()), colored);
    Ok(())
}

fn generate_pbxproj(name: &str, sources: &[(String, String)]) -> String {
    let ids: Vec<String> = (0..sources.len() + 9)
        .map(|i| format!("{:024X}", 0xA1000 + i))
        .collect();
    let project_id = &ids[0];
    let target_id = &ids[1];
    let sources_phase = &ids[2];
    let resources_phase = &ids[3];
    let frameworks_phase = &ids[4];
    let cfg_list = &ids[5];
    let debug_cfg = &ids[6];
    let release_cfg = &ids[7];
    let group_id = &ids[8 + sources.len()];
    let fref = format!("{:024X}", 0xC0000);
    let pconfig_list = format!("{:024X}", 0xD0000);
    let mut child_ids = String::new();

    let mut build_files = String::new();
    let mut file_refs = String::new();
    let mut file_ids = String::new();
    for (i, (file, _)) in sources.iter().enumerate() {
        let bf = &ids[8 + i];
        let fr = format!("{:024X}", 0xB1000 + i);
        build_files.push_str(&format!(
            "\t\t{bf} /* {file} in Sources */ = {{isa = PBXBuildFile; fileRef = {fr} /* {file} */; }};\n"
        ));
        file_refs.push_str(&format!(
            "\t\t{fr} /* {file} */ = {{isa = PBXFileReference; lastKnownFileType = sourcecode; path = \"{file}\"; sourceTree = \"<group>\"; parent = {group_id}; }};\n"
        ));
        child_ids.push_str(&format!("\t\t\t\t{fr} /* {file} */,\n"));
        file_ids.push_str(&format!("\t\t\t\t{bf} /* {file} in Sources */,\n"));
    }
    let source_is_swift = sources
        .first()
        .map(|(f, _)| f.ends_with(".swift"))
        .unwrap_or(false);
    let swift_version = if source_is_swift {
        "\t\t\tSWIFT_VERSION = 5.9;\n"
    } else {
        ""
    };

    format!(
        r#"// !$*UTF8*$!
{{
	archiveVersion = 1;
	classes = {{
	}};
	objectVersion = 56;
	objects = {{

/* Begin PBXBuildFile section */
{build_files}/* End PBXBuildFile section */

/* Begin PBXFileReference section */
{file_refs}/* End PBXFileReference section */

/* Begin PBXGroup section */
	{group_id} /* {name} */ = {{
		isa = PBXGroup;
		children = (
{child_ids}		);
		path = {name};
		sourceTree = "<group>";
	}};
/* End PBXGroup section */

/* Begin PBXFrameworksBuildPhase section */
	{frameworks_phase} /* Frameworks */ = {{
		isa = PBXFrameworksBuildPhase;
		buildActionMask = 2147483647;
		files = (
		);
		runOnlyForDeploymentPostprocessing = 0;
	}};
/* End PBXFrameworksBuildPhase section */

/* Begin PBXNativeTarget section */
	{target_id} /* {name} */ = {{
		isa = PBXNativeTarget;
		buildConfigurationList = {cfg_list} /* Build configuration list for PBXNativeTarget "{name}" */;
		buildPhases = (
			{sources_phase} /* Sources */,
			{frameworks_phase} /* Frameworks */,
			{resources_phase} /* Resources */,
		);
		buildRules = (
		);
		dependencies = (
		);
		name = {name};
		productName = {name};
		productReference = {fref};
		productType = "com.apple.product-type.application";
	}};
/* End PBXNativeTarget section */

/* Begin PBXProject section */
	{project_id} /* Project object */ = {{
		isa = PBXProject;
		attributes = {{ LastUpgradeCheck = 1500; }};
		buildConfigurationList = {pconfig_list};
		compatibilityVersion = "Xcode 14.0";
		developmentRegion = en;
		knownRegions = ( en, Base );
		mainGroup = {group_id};
		productRefGroup = {group_id};
		projectDirPath = "";
		projectRoot = "";
		targets = (
			{target_id} /* {name} */,
		);
	}};
/* End PBXProject section */

/* Begin PBXResourcesBuildPhase section */
	{resources_phase} /* Resources */ = {{
		isa = PBXResourcesBuildPhase;
		buildActionMask = 2147483647;
		files = (
		);
		runOnlyForDeploymentPostprocessing = 0;
	}};
/* End PBXResourcesBuildPhase section */

/* Begin PBXSourcesBuildPhase section */
	{sources_phase} /* Sources */ = {{
		isa = PBXSourcesBuildPhase;
		buildActionMask = 2147483647;
		files = (
{file_ids}		);
		runOnlyForDeploymentPostprocessing = 0;
	}};
/* End PBXSourcesBuildPhase section */

/* Begin XCBuildConfiguration section */
	{debug_cfg} /* Debug */ = {{
		isa = XCBuildConfiguration;
		buildSettings = {{
			IPHONEOS_DEPLOYMENT_TARGET = 16.0;
			INFOPLIST_FILE = "{name}/Info.plist";
			PRODUCT_BUNDLE_IDENTIFIER = com.example.{name};
			PRODUCT_NAME = "$(TARGET_NAME)";
			TARGETED_DEVICE_FAMILY = "1,2";
{swift_version}		}};
		name = Debug;
	}};
	{release_cfg} /* Release */ = {{
		isa = XCBuildConfiguration;
		buildSettings = {{
			IPHONEOS_DEPLOYMENT_TARGET = 16.0;
			INFOPLIST_FILE = "{name}/Info.plist";
			PRODUCT_BUNDLE_IDENTIFIER = com.example.{name};
			PRODUCT_NAME = "$(TARGET_NAME)";
			SWIFT_OPTIMIZATION_LEVEL = "-O";
{swift_version}		}};
		name = Release;
	}};
/* End XCBuildConfiguration section */

/* Begin XCConfigurationList section */
	{cfg_list} /* Build configuration list for PBXNativeTarget "{name}" */ = {{
		isa = XCConfigurationList;
		buildConfigurations = (
			{debug_cfg} /* Debug */,
			{release_cfg} /* Release */,
		);
		defaultConfigurationIsVisible = 0;
		defaultConfigurationName = Release;
	}};
	{pconfig_list} /* Build configuration list for PBXProject "{name}" */ = {{
		isa = XCConfigurationList;
		buildConfigurations = (
		);
		defaultConfigurationIsVisible = 0;
	}};
/* End XCConfigurationList section */
	}};
	rootObject = {project_id} /* Project object */;
}}
"#,
        name = name,
        build_files = build_files,
        file_refs = file_refs,
        file_ids = file_ids,
        child_ids = child_ids,
        fref = fref,
        pconfig_list = pconfig_list,
    )
}

// ------------------------------------------------------------------ build

#[allow(clippy::too_many_arguments)]
fn cmd_build(
    cli: &Cli,
    config: &Config,
    project_path: &Path,
    configuration: Configuration,
    target_name: Option<&str>,
    sdk_name: &str,
    clean: bool,
    jobs: Option<usize>,
) -> Result<()> {
    let colored = color_enabled(cli);
    status("Анализ проекта...", colored);
    let prepared = prepare(cli, config, project_path, configuration, target_name)
        .with_context(|| format!("не удалось разобрать {}", project_path.display()))?;
    status(
        &format!(
            "проект '{}': targets: {}",
            prepared.project.name,
            prepared.project.targets.len()
        ),
        colored,
    );
    let target = &prepared.target;
    let cfg = target
        .configuration(&configuration.to_string())
        .or_else(|| target.configurations.first())
        .ok_or_else(|| anyhow::anyhow!("у target '{}' нет конфигураций", target.name))?;
    let settings = &cfg.settings;
    let project_root = prepared.project_root.clone();
    let toolchain = &prepared.toolchain;
    let sdk_path = prepared.sdk_path.clone();
    let sdk_report = prepared.sdk_report.clone();

    status("Предварительная проверка (Diagnostic Engine)...", colored);
    let mut diagnostics = run_checks(&prepared);
    if sdk_name != "iphoneos" {
        diagnostics.push(
            Diagnostic::error(
                "E0101",
                format!("поддерживается только SDK iphoneos, запрошено: {sdk_name}"),
            )
            .with_hint("используйте --sdk iphoneos"),
        );
    }
    diagnostics::apply_strict(&mut diagnostics, false);
    print_diagnostics(&diagnostics, config, colored);
    if diagnostics::has_errors(&diagnostics) {
        bail!(
            "сборка остановлена до компиляции: {} ошиб(ок). Никакой фиктивный IPA не создаётся.",
            diagnostics
                .iter()
                .filter(|d| d.severity == Severity::Error)
                .count()
        );
    }

    let swift_sources = target.swift_sources();
    let swift_capability = if !swift_sources.is_empty() {
        let swift = SwiftDriver::new(toolchain, sdk_report.clone()).with_verbose(cli.verbose);
        prepared
            .swift_capability
            .clone()
            .map(|capability| (capability, swift))
    } else {
        None
    };

    let build_root = config.build_root(&project_root);
    let obj_dir = build_root.join("objects");
    fs::create_dir_all(&obj_dir)?;

    let cache_file = build::cache_path(config, target, &configuration.to_string());
    let mut cache = if clean {
        BuildCache::default()
    } else {
        BuildCache::load(&cache_file)?
    };
    if clean {
        let _ = fs::remove_dir_all(&build_root);
        fs::create_dir_all(&obj_dir)?;
    }

    let min_os = settings
        .iphoneos_deployment_target
        .clone()
        .unwrap_or_else(|| "16.0".into());

    status("Компиляция C/C++/Objective-C...", colored);
    let clang = ClangDriver::new(toolchain, sdk_path.clone())
        .with_min_os(min_os.clone())
        .with_verbose(cli.verbose);
    let mut objects = Vec::new();
    let mut compiled = Vec::new();
    let mut cached = Vec::new();
    let opts = format!("{:?}", settings).into_bytes();

    let c_sources: Vec<PathBuf> = target
        .c_sources()
        .into_iter()
        .map(|p| {
            if p.is_absolute() {
                p.clone()
            } else {
                project_root.join(p)
            }
        })
        .filter(|p| p.exists())
        .collect();

    let num_threads = jobs.unwrap_or_else(|| {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
    });
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .context("не удалось создать пул потоков сборки")?;

    let tasks: Vec<(usize, PathBuf, PathBuf, String, bool)> = c_sources
        .iter()
        .enumerate()
        .map(|(i, src)| {
            let obj = obj_dir.join(format!(
                "{:03}_{}.o",
                i,
                src.file_stem().unwrap().to_string_lossy()
            ));
            let key = format!("c:{}:{}", src.display(), configuration);
            let fresh = !clean && cache.is_fresh(&key, src, &opts, &obj).unwrap_or(false);
            (i, src.clone(), obj, key, fresh)
        })
        .collect();

    let outcomes: Vec<(usize, anyhow::Result<String>)> = pool.install(|| {
        tasks
            .par_iter()
            .map(|(i, src, obj, _, fresh)| {
                if *fresh {
                    return (*i, Ok(String::new()));
                }
                match clang.compile(src, obj, settings) {
                    Ok(result) => (*i, Ok(result.command)),
                    Err(e) => (*i, Err(e)),
                }
            })
            .collect()
    });

    for (i, src, obj, key, fresh) in &tasks {
        let outcome = outcomes
            .iter()
            .find(|(idx, _)| idx == i)
            .map(|(_, r)| r.as_ref());
        match outcome {
            Some(Ok(command)) if *fresh => {
                cached.push(obj.clone());
                if config.verbose {
                    eprintln!("  [cache] {}", src.display());
                }
                let _ = command;
            }
            Some(Ok(command)) => {
                compiled.push(obj.clone());
                if config.verbose {
                    eprintln!("  [clang] {command}");
                }
            }
            Some(Err(e)) => {
                let message = compiler_error_message(e, config, colored, cli.verbose);
                return Err(anyhow::anyhow!(
                    "ошибка компиляции {}: {message}",
                    src.display()
                ));
            }
            None => {}
        }
        cache.record(key, src, &opts, obj)?;
        objects.push(obj.clone());
    }
    cache.save(&cache_file)?;

    if let Some((capability, swift)) = &swift_capability {
        if capability.can_target_ios {
            status("Компиляция Swift...", colored);
            for (i, src) in swift_sources.iter().enumerate() {
                let source = if src.is_absolute() {
                    src.to_path_buf()
                } else {
                    project_root.join(src)
                };
                if !source.exists() {
                    continue;
                }
                let obj = obj_dir.join(format!(
                    "swift_{:03}_{}.o",
                    i,
                    src.file_stem().unwrap().to_string_lossy()
                ));
                let out = swift.compile_object(&source, &obj)?;
                let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
                if !out.status.success() {
                    print_compiler_output(&stderr, config, colored, cli.verbose);
                    bail!(
                        "компиляция Swift {} завершилась с кодом {}",
                        source.display(),
                        out.status.code().unwrap_or(-1)
                    );
                }
                if !obj.exists() {
                    bail!("swiftc не создал объектный файл {}", obj.display());
                }
                objects.push(obj);
            }
        }
    }

    status("Линковка Mach-O (ld64.lld)...", colored);
    let sdk_version = sdk_report
        .as_ref()
        .and_then(|r| r.version.clone())
        .unwrap_or_else(|| min_os.clone());
    let framework_search_paths: Vec<PathBuf> = settings
        .framework_search_paths
        .iter()
        .map(|p| {
            let path = PathBuf::from(p);
            if path.is_absolute() {
                path
            } else {
                project_root.join(path)
            }
        })
        .collect();
    let linker = Linker::new(toolchain, sdk_path.clone(), min_os.clone(), sdk_version)
        .with_frameworks(target.frameworks.clone())
        .with_framework_search_paths(framework_search_paths)
        .with_other_ldflags(settings.other_ldflags.clone())
        .with_swift_runtime(
            swift_capability
                .as_ref()
                .map(|(c, _)| c.can_target_ios && !swift_sources.is_empty())
                .unwrap_or(false),
        );
    let exe = build_root.join(&target.name);
    let link = match linker.link(&objects, &exe) {
        Ok(result) => result,
        Err(e) => {
            let message = compiler_error_message(&e, config, colored, cli.verbose);
            bail!("{message}");
        }
    };
    diagnostics.extend(link.diagnostics);
    if !link.loadable_on_ios {
        bail!(
            "получен структурно корректный ARM64 Mach-O, но без SDK он не может быть загружен iOS.\n\
             Подключите iPhoneOS.sdk и повторите сборку. OpenIPA не выдаёт такой бинарник за рабочее приложение."
        );
    }

    status("Обработка ресурсов...", colored);
    let mut catalog_diags = Vec::new();
    let mut resource_files = target.resources.clone();
    let mut app_icons: Vec<String> = Vec::new();
    let mut launch_images: Vec<String> = Vec::new();
    for res in &target.resources {
        let full = if res.is_absolute() {
            res.clone()
        } else {
            project_root.join(res)
        };
        if full.is_dir() && full.file_name().and_then(|s| s.to_str()) == Some("Assets.xcassets") {
            let catalog = assets::compile_catalog(&full)?;
            catalog_diags.extend(catalog.diagnostics);
            app_icons.extend(catalog.app_icons.clone());
            launch_images.extend(catalog.launch_images.clone());
            for item in &catalog.items {
                let dst_dir = build_root.join("resources");
                fs::create_dir_all(&dst_dir)?;
                fs::copy(&item.source, dst_dir.join(&item.output_name))?;
            }
            resource_files.push(build_root.join("resources"));
        }
    }
    print_diagnostics(&catalog_diags, config, colored);

    let mut plist = match settings
        .info_plist_file
        .as_ref()
        .map(|p| project_root.join(p))
        .filter(|p| p.exists())
    {
        Some(p) => info_plist::read_dict(&p)?,
        None => default_info_plist(
            &target.name,
            &bundle_id(settings, target),
            &min_os,
            &target.name,
        ),
    };
    plist
        .entry("CFBundleExecutable".into())
        .or_insert_with(|| plist::Value::String(target.name.clone()));
    let launch_image = launch_images
        .first()
        .map(|s| {
            Path::new(s)
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or(s)
        })
        .map(str::to_string)
        .or_else(|| {
            target
                .resources
                .iter()
                .any(|r| r.to_string_lossy().contains("LaunchScreen"))
                .then(|| "LaunchScreen".to_string())
        });
    packager::augment_info_plist(
        &mut plist,
        &target.name,
        &min_os,
        sdk_report.as_ref().and_then(|r| r.version.as_deref()),
        &app_icons,
        launch_image.as_deref(),
    );
    if let Some(bundle_id) = settings.product_bundle_identifier.as_ref() {
        plist.insert(
            "CFBundleIdentifier".into(),
            plist::Value::String(bundle_id.clone()),
        );
    }

    status("Сборка .app и упаковка IPA...", colored);
    let app_dir = build_root.join(format!("{}.app", target.name));
    let frameworks: Vec<PathBuf> = target
        .frameworks
        .iter()
        .map(|f| project_root.join(f))
        .filter(|p| p.exists())
        .collect();
    let options = BundleOptions {
        app_name: &target.name,
        executable: &exe,
        info_plist: &plist,
        resources: &resource_files,
        project_root: &project_root,
        frameworks: &frameworks,
        plugins: &[],
    };
    let _file_count = packager::make_app_bundle(&app_dir, &options)?;
    let ipa = build_root.join(format!("{}.ipa", target.name));
    let report = packager::package_ipa(&app_dir, &ipa, &target.name)?;

    success(&ipa.display().to_string(), colored);
    eprintln!(
        "  скомпилировано: {}, из кэша: {}, зависимостей dylib: {}",
        compiled.len(),
        cached.len(),
        link.info.dylibs.len()
    );
    let _ = report;
    let _ = rayon::current_num_threads();
    Ok(())
}

fn bundle_id(
    settings: &openipa::project::BuildSettings,
    target: &openipa::project::Target,
) -> String {
    settings
        .product_bundle_identifier
        .clone()
        .unwrap_or_else(|| format!("com.example.{}", target.name))
}

// -------------------------------------------------------- inspect/validate

fn cmd_inspect(cli: &Cli, _config: &Config, ipa: &Path) -> Result<()> {
    let colored = color_enabled(cli);
    let file =
        fs::File::open(ipa).with_context(|| format!("не удалось открыть {}", ipa.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).with_context(|| format!("{} не ZIP", ipa.display()))?;
    println!("IPA: {}", ipa.display());
    println!("Записей: {}", archive.len());
    let mut total = 0u64;
    for i in 0..archive.len() {
        let e = archive.by_index(i)?;
        total += e.size();
        if i < 40 {
            println!("  {} ({} байт)", e.name(), e.size());
        }
    }
    println!("Общий распакованный размер: {total} байт");
    match packager::read_info_plist_from_ipa(ipa) {
        Ok(p) => {
            println!(
                "Bundle ID: {:?}",
                packager::plist_string(&p, "CFBundleIdentifier")
            );
            println!(
                "Executable: {:?}",
                packager::plist_string(&p, "CFBundleExecutable")
            );
        }
        Err(e) => eprintln!("Info.plist: {e}"),
    }
    let _ = colored;
    Ok(())
}

fn cmd_validate(cli: &Cli, config: &Config, ipa: &Path) -> Result<()> {
    let colored = color_enabled(cli);
    let report = validator::validate_ipa(ipa)?;
    if config.output_format == OutputFormat::Json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for c in &report.checks {
            let mark = if c.ok {
                paint("OK", "1;32", colored)
            } else {
                paint("FAIL", "1;31", colored)
            };
            eprintln!("  [{mark}] {} — {}", c.id, c.message);
        }
        eprintln!("  {}", paint(&report.disclaimer, "1;33", colored));
        if report.valid {
            success(
                &format!("{} проходит статическую проверку", ipa.display()),
                colored,
            );
        } else {
            fail(&format!("{} не прошёл проверку", ipa.display()), colored);
        }
    }
    if !report.valid {
        bail!("валидация не пройдена");
    }
    Ok(())
}

fn cmd_clean(cli: &Cli, _config: &Config, project: &Path) -> Result<()> {
    let colored = color_enabled(cli);
    let target = if project.extension().and_then(|e| e.to_str()) == Some("xcodeproj") {
        project
            .parent()
            .map(|p| p.join("build"))
            .unwrap_or_else(|| PathBuf::from("build"))
    } else {
        project.join("build")
    };
    if target.exists() {
        fs::remove_dir_all(&target)?;
        status(&format!("удалено {}", target.display()), colored);
    } else {
        status("чистить нечего", colored);
    }
    Ok(())
}

// ------------------------------------------------------------------ doctor

fn cmd_doctor(cli: &Cli, config: &Config) -> Result<()> {
    let colored = color_enabled(cli);
    let toolchain = Toolchain::detect()?;
    let mut diagnostics = Vec::new();
    let line = |label: &str, value: &str, ok: bool| {
        let mark = if ok {
            paint("OK", "1;32", colored)
        } else {
            paint("MISSING", "1;31", colored)
        };
        eprintln!("  [{mark}] {label}: {value}");
    };
    line("Rust", rustc_version().as_deref().unwrap_or("?"), true);
    line(
        "clang",
        toolchain.clang_version.as_deref().unwrap_or("не найден"),
        toolchain.can_compile_c(),
    );
    line(
        "clang++",
        toolchain
            .clangpp
            .as_ref()
            .map(|p| p.display().to_string())
            .as_deref()
            .unwrap_or("не найден"),
        toolchain.clangpp.is_some(),
    );
    let swift_label = match (
        &toolchain.swift_version,
        std::env::var_os("OPENIPA_SWIFT_TOOLCHAIN"),
    ) {
        (Some(v), Some(root)) => format!(
            "{v} (OPENIPA_SWIFT_TOOLCHAIN={})",
            PathBuf::from(root).display()
        ),
        (Some(v), None) => v.clone(),
        (None, _) => "не найден".into(),
    };
    line("swiftc", &swift_label, toolchain.can_compile_swift());
    line(
        "ld64.lld",
        toolchain
            .ld64_lld
            .as_ref()
            .map(|p| p.display().to_string())
            .as_deref()
            .unwrap_or("не найден"),
        toolchain.can_link_macho(),
    );
    line(
        "zip",
        toolchain
            .zip
            .as_ref()
            .map(|p| p.display().to_string())
            .as_deref()
            .unwrap_or("не найден"),
        toolchain.zip.is_some(),
    );
    line(
        "actool (Apple)",
        toolchain
            .actool
            .as_ref()
            .map(|p| p.display().to_string())
            .as_deref()
            .unwrap_or("недоступен на Linux"),
        false,
    );
    line(
        "host",
        &format!("{}-{}", toolchain.host_arch, toolchain.host_os),
        true,
    );

    let registry = SdkRegistry::load(config)?;
    let mut active_sdk_report = None;
    if let Some(active) = registry.active.as_deref() {
        let entry = registry.sdks.get(active);
        line("активный SDK", active, entry.is_some());
        if let Some(e) = entry {
            let report = sdk::inspect_sdk(&e.path)?;
            line("SDK struct", &e.path.display().to_string(), report.valid);
            line(
                "SDK can link",
                if report.can_link { "да" } else { "нет" },
                report.can_link,
            );
            active_sdk_report = Some(report);
        }
    } else {
        line(
            "iOS SDK",
            "не назначен (openipa sdk add /path/to/iPhoneOS.sdk)",
            false,
        );
    }

    let swift = SwiftDriver::new(&toolchain, active_sdk_report).capability();
    diagnostics.extend(swift.diagnostics);
    print_diagnostics(&diagnostics, config, colored);
    eprintln!(
        "  {}",
        paint(
            "OpenIPA не требует macOS/Xcode для C/Objective-C. Swift/SwiftUI требуют Apple SDK.",
            "1;34",
            colored
        )
    );
    Ok(())
}

fn rustc_version() -> Option<String> {
    let out = ProcessCommand::new("rustc")
        .arg("--version")
        .output()
        .ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// --------------------------------------------------------------------- sdk

fn cmd_sdk(cli: &Cli, config: &Config, action: &SdkCommand) -> Result<()> {
    let colored = color_enabled(cli);
    let mut registry = SdkRegistry::load(config)?;
    match action {
        SdkCommand::List => {
            if registry.sdks.is_empty() {
                eprintln!(
                    "  SDK не зарегистрированы. Добавьте: openipa sdk add /path/to/iPhoneOS.sdk"
                );
            }
            for (name, entry) in &registry.sdks {
                let active = registry.active.as_deref() == Some(name.as_str());
                eprintln!(
                    "  {}{} — {} ({})",
                    if active { "* " } else { "  " },
                    name,
                    entry.path.display(),
                    entry.version.as_deref().unwrap_or("version ?")
                );
            }
            Ok(())
        }
        SdkCommand::Add { path, name } => {
            let report = sdk::inspect_sdk(path)?;
            print_diagnostics(&report.diagnostics, config, colored);
            if !report.valid {
                bail!("SDK не прошёл проверку и не добавлен");
            }
            let name = name.clone().unwrap_or_else(|| {
                path.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("sdk")
                    .to_string()
            });
            let entry = openipa::sdk::SdkEntry {
                name: name.clone(),
                path: path.clone(),
                version: report.version.clone(),
                display_name: report.display_name.clone(),
                canonical_name: report.canonical_name.clone(),
                added_at: chrono::Utc::now().to_rfc3339(),
            };
            registry.sdks.insert(name.clone(), entry);
            if registry.active.is_none() {
                registry.active = Some(name.clone());
            }
            registry.save(config)?;
            success(&format!("SDK '{name}' добавлен"), colored);
            Ok(())
        }
        SdkCommand::Inspect { name } | SdkCommand::Validate { name } => {
            let entry = registry.resolve(name, config)?;
            let report = sdk::inspect_sdk(&entry.path)?;
            if config.output_format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                eprintln!("SDK: {}", report.path.display());
                eprintln!("  Version: {:?}", report.version);
                eprintln!("  DisplayName: {:?}", report.display_name);
                eprintln!("  CanonicalName: {:?}", report.canonical_name);
                eprintln!("  MinDeploymentTarget: {:?}", report.min_deployment_target);
                eprintln!(
                    "  libSystem: {:?}",
                    report.libsystem.as_ref().map(|p| p.display().to_string())
                );
                eprintln!(
                    "  Swift stdlib (iOS): {:?}",
                    report
                        .swift_stdlib_iphoneos
                        .as_ref()
                        .map(|p| p.display().to_string())
                );
                eprintln!(
                    "  Годен для линковки: {}, для Swift/iOS: {}",
                    report.can_link, report.can_compile_swift_ios
                );
                eprintln!("  Framework binaries: {}", report.framework_binaries.len());
                for c in &report.checks {
                    eprintln!(
                        "  [{}] {} — {}",
                        if c.ok {
                            paint("OK", "1;32", colored)
                        } else {
                            paint("FAIL", "1;31", colored)
                        },
                        c.name,
                        c.detail
                    );
                }
                print_diagnostics(&report.diagnostics, config, colored);
            }
            if !report.valid {
                bail!("SDK не прошёл проверку");
            }
            Ok(())
        }
        SdkCommand::Remove { name } => {
            if registry.sdks.remove(name).is_none() {
                bail!("SDK '{name}' не найден");
            }
            if registry.active.as_deref() == Some(name.as_str()) {
                registry.active = registry.sdks.keys().next().cloned();
            }
            registry.save(config)?;
            success(&format!("SDK '{name}' удалён из реестра"), colored);
            Ok(())
        }
        SdkCommand::Use { name } => {
            if !registry.sdks.contains_key(name) {
                bail!("SDK '{name}' не найден");
            }
            registry.active = Some(name.clone());
            registry.save(config)?;
            success(&format!("активный SDK: {name}"), colored);
            Ok(())
        }
    }
}

// ------------------------------------------------------------------ package

fn cmd_package(cli: &Cli, config: &Config, action: &PackageCommand) -> Result<()> {
    let colored = color_enabled(cli);
    let project = match action {
        PackageCommand::Resolve { project }
        | PackageCommand::Update { project }
        | PackageCommand::List { project } => project,
    };
    let manifest = project.join("Package.swift");
    let resolved = project.join("Package.resolved");
    let mut diagnostics = Vec::new();
    let mut deps: Vec<(String, String)> = Vec::new();

    if resolved.exists() {
        let json: serde_json::Value = serde_json::from_str(&fs::read_to_string(&resolved)?)?;
        if let Some(pins) = json.get("pins").and_then(|v| v.as_array()) {
            for pin in pins {
                let id = pin.get("identity").and_then(|v| v.as_str()).unwrap_or("?");
                let ver = pin
                    .get("state")
                    .and_then(|s| s.get("version"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                deps.push((id.into(), ver.into()));
            }
        }
    } else if manifest.exists() {
        diagnostics.push(
            Diagnostic::warning(
                "NO_PACKAGE_RESOLVED",
                "Package.swift найден, но Package.resolved отсутствует",
            )
            .with_hint("запустите `swift package resolve` в среде с Xcode (SwiftPM на Linux не резолвит iOS-зависимости)"),
        );
    } else {
        diagnostics.push(Diagnostic::note(
            "NO_SWIFT_PACKAGES",
            "Swift Package манифест не найден",
        ));
    }
    print_diagnostics(&diagnostics, config, colored);
    match action {
        PackageCommand::List { .. } => {
            for (name, ver) in &deps {
                eprintln!("  {name} @ {ver}");
            }
            if deps.is_empty() {
                eprintln!("  зависимостей нет");
            }
        }
        PackageCommand::Resolve { .. } => {
            status(&format!("разрешено зависимостей: {}", deps.len()), colored);
        }
        PackageCommand::Update { .. } => {
            diagnostics.push(Diagnostic::warning(
                "PACKAGE_UPDATE_REQUIRES_SWIFTPM",
                "обновление зависимостей выполняется SwiftPM, который на Linux не поддерживает iOS-таргет",
            ));
            print_diagnostics(&diagnostics, config, colored);
        }
    }
    let _ = colored;
    Ok(())
}
