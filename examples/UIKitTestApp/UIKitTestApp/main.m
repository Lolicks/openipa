// UIKitTestApp — настоящий UIKit-проект для проверки портирования на OpenIPA.
// 1) UILabel, 2) UIButton, 3) target-action, 4) два экрана через
// UINavigationController, 5) загрузка ресурсов из .app через NSBundle.

#define nil 0
typedef signed char BOOL;
#define YES 1
#define NO 0

struct CGPoint { double x; double y; };
struct CGSize { double width; double height; };
struct CGRect { struct CGPoint origin; struct CGSize size; };

@class UINavigationController;

@interface NSObject
+ (instancetype)alloc;
- (instancetype)init;
@end

@interface NSString : NSObject
+ (NSString *)stringWithContentsOfFile:(NSString *)path encoding:(unsigned long)encoding error:(id *)error;
@end

@interface NSBundle : NSObject
+ (NSBundle *)mainBundle;
- (NSString *)pathForResource:(NSString *)name ofType:(NSString *)extension;
@end

@interface UIColor : NSObject
+ (UIColor *)whiteColor;
+ (UIColor *)blackColor;
+ (UIColor *)systemBlueColor;
+ (UIColor *)systemGreenColor;
@end

@interface UIFont : NSObject
+ (UIFont *)systemFontOfSize:(double)size;
+ (UIFont *)boldSystemFontOfSize:(double)size;
@end

@interface UIResponder : NSObject
@end

@interface UIApplication : UIResponder
@end

@interface UIView : UIResponder
- (instancetype)initWithFrame:(struct CGRect)frame;
- (void)addSubview:(UIView *)view;
@property (nonatomic, assign) struct CGRect frame;
@property (nonatomic, assign) struct CGRect bounds;
@property (nonatomic, assign) UIColor *backgroundColor;
@end

@interface UIImage : NSObject
+ (UIImage *)imageWithContentsOfFile:(NSString *)path;
@end

@interface UIImageView : UIView
@property (nonatomic, assign) UIImage *image;
@property (nonatomic, assign) long contentMode;
@end

@interface UILabel : UIView
@property (nonatomic, assign) NSString *text;
@property (nonatomic, assign) UIColor *textColor;
@property (nonatomic, assign) UIFont *font;
@property (nonatomic, assign) long textAlignment;
@property (nonatomic, assign) long numberOfLines;
@end

@interface UIButton : UIView
+ (UIButton *)buttonWithType:(long)type;
- (void)setTitle:(NSString *)title forState:(unsigned long)state;
- (void)setTitleColor:(UIColor *)color forState:(unsigned long)state;
- (void)addTarget:(id)target action:(SEL)action forControlEvents:(unsigned long)events;
@end

@interface UIViewController : UIResponder
- (void)viewDidLoad;
@property (nonatomic, readonly) UIView *view;
@property (nonatomic, readonly) UINavigationController *navigationController;
@property (nonatomic, assign) NSString *title;
@end

@interface UINavigationController : UIViewController
- (instancetype)initWithRootViewController:(UIViewController *)controller;
- (void)pushViewController:(UIViewController *)controller animated:(BOOL)animated;
- (UIViewController *)popViewControllerAnimated:(BOOL)animated;
@end

@interface UIScreen : NSObject
+ (UIScreen *)mainScreen;
- (struct CGRect)bounds;
@end

@interface UIWindow : UIView
- (void)makeKeyAndVisible;
@property (nonatomic, assign) UIViewController *rootViewController;
@end

int UIApplicationMain(int argc, char *argv[], NSString *principalClassName, NSString *delegateClassName);

@interface DetailViewController : UIViewController
@end

@implementation DetailViewController
- (void)viewDidLoad {
    [super viewDidLoad];
    self.title = @"Detail";
    self.view.backgroundColor = [UIColor whiteColor];

    struct CGRect b = self.view.bounds;
    NSBundle *bundle = [NSBundle mainBundle];
    NSString *imagePath = [bundle pathForResource:@"banner" ofType:@"png"];
    NSString *textPath = [bundle pathForResource:@"message" ofType:@"txt"];
    NSString *message = [NSString stringWithContentsOfFile:textPath encoding:4 error:nil];

    struct CGRect imageFrame;
    imageFrame.origin.x = 20;
    imageFrame.origin.y = 100;
    imageFrame.size.width = b.size.width - 40;
    imageFrame.size.height = 120;
    UIImageView *imageView = [[UIImageView alloc] initWithFrame:imageFrame];
    imageView.image = [UIImage imageWithContentsOfFile:imagePath];
    imageView.contentMode = 1;
    [self.view addSubview:imageView];

    struct CGRect labelFrame;
    labelFrame.origin.x = 20;
    labelFrame.origin.y = 240;
    labelFrame.size.width = b.size.width - 40;
    labelFrame.size.height = 120;
    UILabel *label = [[UILabel alloc] initWithFrame:labelFrame];
    label.text = message;
    label.textColor = [UIColor blackColor];
    label.font = [UIFont systemFontOfSize:22.0];
    label.numberOfLines = 0;
    label.textAlignment = 1;
    [self.view addSubview:label];

    UIButton *back = [UIButton buttonWithType:0];
    struct CGRect backFrame;
    backFrame.origin.x = 20;
    backFrame.origin.y = 400;
    backFrame.size.width = b.size.width - 40;
    backFrame.size.height = 50;
    back.frame = backFrame;
    [back setTitle:@"Back" forState:0];
    [back setTitleColor:[UIColor systemBlueColor] forState:0];
    [back addTarget:self action:@selector(goBack) forControlEvents:64];
    [self.view addSubview:back];
}

- (void)goBack {
    [self.navigationController popViewControllerAnimated:YES];
}
@end

@interface HelloViewController : UIViewController
@end

@implementation HelloViewController
- (void)viewDidLoad {
    [super viewDidLoad];
    self.title = @"OpenIPA";
    self.view.backgroundColor = [UIColor whiteColor];

    struct CGRect b = self.view.bounds;
    struct CGRect labelFrame;
    labelFrame.origin.x = 20;
    labelFrame.origin.y = 160;
    labelFrame.size.width = b.size.width - 40;
    labelFrame.size.height = 60;
    UILabel *label = [[UILabel alloc] initWithFrame:labelFrame];
    label.text = @"Hello, World!";
    label.textColor = [UIColor blackColor];
    label.font = [UIFont boldSystemFontOfSize:32.0];
    label.textAlignment = 1;
    [self.view addSubview:label];

    UIButton *open = [UIButton buttonWithType:0];
    struct CGRect buttonFrame;
    buttonFrame.origin.x = 20;
    buttonFrame.origin.y = 260;
    buttonFrame.size.width = b.size.width - 40;
    buttonFrame.size.height = 50;
    open.frame = buttonFrame;
    [open setTitle:@"Open Detail" forState:0];
    [open setTitleColor:[UIColor systemBlueColor] forState:0];
    [open addTarget:self action:@selector(openDetail) forControlEvents:64];
    [self.view addSubview:open];
}

- (void)openDetail {
    DetailViewController *detail = [[DetailViewController alloc] init];
    [self.navigationController pushViewController:detail animated:YES];
}
@end

@interface AppDelegate : UIResponder
@property (nonatomic, assign) UIWindow *window;
@end

@implementation AppDelegate
- (BOOL)application:(UIApplication *)application didFinishLaunchingWithOptions:(id)options {
    struct CGRect bounds = [[UIScreen mainScreen] bounds];
    UIWindow *window = [[UIWindow alloc] initWithFrame:bounds];
    HelloViewController *root = [[HelloViewController alloc] init];
    UINavigationController *navigation = [[UINavigationController alloc] initWithRootViewController:root];
    window.rootViewController = navigation;
    [window makeKeyAndVisible];
    self.window = window;
    return YES;
}
@end

int main(int argc, char *argv[]) {
    return UIApplicationMain(argc, argv, nil, @"AppDelegate");
}
