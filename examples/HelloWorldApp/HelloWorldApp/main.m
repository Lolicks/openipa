#define nil 0
typedef signed char BOOL;
#define YES 1
#define NO 0

struct CGPoint { double x; double y; };
struct CGSize { double width; double height; };
struct CGRect { struct CGPoint origin; struct CGSize size; };

@interface NSObject
+ (instancetype)alloc;
- (instancetype)init;
@end

@interface NSString : NSObject
@end

@interface UIColor : NSObject
+ (UIColor *)whiteColor;
+ (UIColor *)blackColor;
@end

@interface UIFont : NSObject
+ (UIFont *)systemFontOfSize:(double)size;
@end

@interface UIResponder : NSObject
@end

@interface UIApplication : UIResponder
@end

@interface UIView : UIResponder
- (instancetype)initWithFrame:(struct CGRect)frame;
- (void)addSubview:(UIView *)view;
@property (nonatomic) struct CGRect frame;
@property (nonatomic) struct CGRect bounds;
@property (nonatomic, assign) UIColor *backgroundColor;
@end

@interface UILabel : UIView
@property (nonatomic, assign) NSString *text;
@property (nonatomic, assign) UIColor *textColor;
@property (nonatomic, assign) UIFont *font;
@property (nonatomic) long textAlignment;
@end

@interface UIScreen : NSObject
+ (UIScreen *)mainScreen;
- (struct CGRect)bounds;
@end

@interface UIViewController : UIResponder
- (void)viewDidLoad;
@property (nonatomic, readonly) UIView *view;
@end

@interface UIWindow : UIView
- (void)makeKeyAndVisible;
@property (nonatomic, assign) UIViewController *rootViewController;
@end

int UIApplicationMain(int argc, char *argv[], NSString *principalClassName, NSString *delegateClassName);

@interface HelloViewController : UIViewController
@end

@implementation HelloViewController
- (void)viewDidLoad {
    [super viewDidLoad];
    self.view.backgroundColor = [UIColor whiteColor];
    struct CGRect b = self.view.bounds;
    struct CGRect frame;
    frame.origin.x = 20;
    frame.origin.y = b.size.height / 2 - 40;
    frame.size.width = b.size.width - 40;
    frame.size.height = 80;
    UILabel *label = [[UILabel alloc] initWithFrame:frame];
    label.text = @"Hello, World!";
    label.textColor = [UIColor blackColor];
    label.font = [UIFont systemFontOfSize:32.0];
    label.textAlignment = 1;
    [self.view addSubview:label];
}
@end

@interface AppDelegate : UIResponder
@property (nonatomic, assign) UIWindow *window;
@end

@implementation AppDelegate
- (BOOL)application:(UIApplication *)application didFinishLaunchingWithOptions:(id)options {
    struct CGRect bounds = [[UIScreen mainScreen] bounds];
    UIWindow *window = [[UIWindow alloc] initWithFrame:bounds];
    window.rootViewController = [[HelloViewController alloc] init];
    [window makeKeyAndVisible];
    self.window = window;
    return YES;
}
@end

int main(int argc, char *argv[]) {
    return UIApplicationMain(argc, argv, nil, @"AppDelegate");
}
