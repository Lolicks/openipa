// FlappyBirdApp — простая игра на настоящих UIKit + CoreGraphics.
// Управление: касание экрана — взмах крыльев; после проигрыша — рестарт.

#define nil 0
typedef signed char BOOL;
#define YES 1
#define NO 0

typedef const struct CGColor *CGColorRef;
typedef struct CGContext *CGContextRef;

struct CGPoint { double x; double y; };
struct CGSize { double width; double height; };
struct CGRect { struct CGPoint origin; struct CGSize size; };

CGContextRef UIGraphicsGetCurrentContext(void);
void CGContextSetFillColorWithColor(CGContextRef context, CGColorRef color);
void CGContextFillRect(CGContextRef context, struct CGRect rect);
void CGContextFillEllipseInRect(CGContextRef context, struct CGRect rect);
void CGContextSetRGBFillColor(CGContextRef context, double r, double g, double b, double a);

@class UINavigationController;

@interface NSObject
+ (instancetype)alloc;
- (instancetype)init;
@end

@interface NSString : NSObject
+ (NSString *)stringWithFormat:(NSString *)format, ...;
@end

@interface NSTimer : NSObject
+ (NSTimer *)scheduledTimerWithTimeInterval:(double)interval target:(id)target selector:(SEL)selector userInfo:(id)userInfo repeats:(BOOL)repeats;
@end

@interface UIColor : NSObject
+ (UIColor *)whiteColor;
+ (UIColor *)blackColor;
+ (UIColor *)colorWithRed:(double)red green:(double)green blue:(double)blue alpha:(double)alpha;
- (CGColorRef)CGColor;
@end

@interface UIFont : NSObject
+ (UIFont *)boldSystemFontOfSize:(double)size;
@end

@interface UIResponder : NSObject
@end

@interface UIApplication : UIResponder
@end

@interface UIView : UIResponder
- (instancetype)initWithFrame:(struct CGRect)frame;
- (void)addSubview:(UIView *)view;
- (void)setNeedsDisplay;
@property (nonatomic, assign) struct CGRect frame;
@property (nonatomic, assign) struct CGRect bounds;
@property (nonatomic, assign) UIColor *backgroundColor;
@end

@interface UILabel : UIView
@property (nonatomic, assign) NSString *text;
@property (nonatomic, assign) UIColor *textColor;
@property (nonatomic, assign) UIFont *font;
@property (nonatomic, assign) long textAlignment;
@end

@interface UIViewController : UIResponder
- (void)viewDidLoad;
@property (nonatomic, readonly) UIView *view;
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

@interface GameView : UIView {
    double birdY;
    double velocity;
    double pipeX;
    double pipeTop;
    int score;
    int gameOver;
    int tickCount;
    UILabel *scoreLabel;
}
- (void)setScoreLabel:(UILabel *)label;
@end

@implementation GameView
- (instancetype)initWithFrame:(struct CGRect)frame {
    self = [super initWithFrame:frame];
    birdY = frame.size.height / 2;
    velocity = 0;
    pipeX = frame.size.width;
    pipeTop = 120;
    score = 0;
    gameOver = 0;
    tickCount = 0;
    [NSTimer scheduledTimerWithTimeInterval:0.02 target:self selector:@selector(tick) userInfo:nil repeats:YES];
    return self;
}

- (void)setScoreLabel:(UILabel *)label {
    scoreLabel = label;
    [scoreLabel setText:@"Score: 0"];
}

- (void)tick {
    struct CGRect bounds = self.bounds;
    tickCount++;
    if (!gameOver) {
        velocity += 0.32;
        birdY += velocity;
        pipeX -= 3.0;
        if (pipeX < -60) {
            pipeX = bounds.size.width + 20;
            // Псевдослучайная высота прохода без вызовов extern-функций.
            pipeTop = 70 + ((tickCount * 37) % 160);
            score++;
            [scoreLabel setText:[NSString stringWithFormat:@"Score: %d", score]];
        }
        double birdLeft = 60;
        double birdRight = 100;
        double gapBottom = pipeTop + 150;
        if (birdY > bounds.size.height - 40 || birdY < 0) {
            gameOver = 1;
        }
        if (birdRight > pipeX && birdLeft < pipeX + 60) {
            if (birdY < pipeTop || birdY + 30 > gapBottom) {
                gameOver = 1;
            }
        }
    }
    [self setNeedsDisplay];
}

- (void)touchesBegan:(id)touches withEvent:(id)event {
    if (gameOver) {
        birdY = self.bounds.size.height / 2;
        velocity = 0;
        pipeX = self.bounds.size.width;
        score = 0;
        gameOver = 0;
        [scoreLabel setText:@"Score: 0"];
    } else {
        velocity = -6.0;
    }
}

- (void)drawRect:(struct CGRect)rect {
    CGContextRef context = UIGraphicsGetCurrentContext();
    UIColor *sky = [UIColor colorWithRed:0.53 green:0.81 blue:0.92 alpha:1.0];
    UIColor *pipeColor = [UIColor colorWithRed:0.13 green:0.55 blue:0.13 alpha:1.0];
    UIColor *birdColor = [UIColor colorWithRed:1.0 green:0.75 blue:0.1 alpha:1.0];

    CGContextSetFillColorWithColor(context, [sky CGColor]);
    CGContextFillRect(context, rect);

    struct CGRect upperPipe;
    upperPipe.origin.x = pipeX;
    upperPipe.origin.y = 0;
    upperPipe.size.width = 60;
    upperPipe.size.height = pipeTop;
    struct CGRect lowerPipe;
    lowerPipe.origin.x = pipeX;
    lowerPipe.origin.y = pipeTop + 150;
    lowerPipe.size.width = 60;
    lowerPipe.size.height = rect.size.height - lowerPipe.origin.y;

    CGContextSetFillColorWithColor(context, [pipeColor CGColor]);
    CGContextFillRect(context, upperPipe);
    CGContextFillRect(context, lowerPipe);

    CGContextSetFillColorWithColor(context, [birdColor CGColor]);
    struct CGRect bird;
    bird.origin.x = 60;
    bird.origin.y = birdY;
    bird.size.width = 40;
    bird.size.height = 30;
    CGContextFillEllipseInRect(context, bird);

    CGContextSetRGBFillColor(context, 0.6, 0.45, 0.25, 1.0);
    struct CGRect ground;
    ground.origin.x = 0;
    ground.origin.y = rect.size.height - 40;
    ground.size.width = rect.size.width;
    ground.size.height = 40;
    CGContextFillRect(context, ground);

    if (gameOver) {
        CGContextSetRGBFillColor(context, 0.0, 0.0, 0.0, 1.0);
        struct CGRect banner;
        banner.origin.x = 20;
        banner.origin.y = rect.size.height / 2 - 40;
        banner.size.width = rect.size.width - 40;
        banner.size.height = 8;
        CGContextFillRect(context, banner);
    }
}
@end

@interface GameViewController : UIViewController
@end

@implementation GameViewController
- (void)viewDidLoad {
    [super viewDidLoad];
    self.view.backgroundColor = [UIColor whiteColor];
    struct CGRect b = self.view.bounds;

    struct CGRect labelFrame;
    labelFrame.origin.x = 0;
    labelFrame.origin.y = 40;
    labelFrame.size.width = b.size.width;
    labelFrame.size.height = 40;
    UILabel *label = [[UILabel alloc] initWithFrame:labelFrame];
    label.text = @"Score: 0";
    label.textColor = [UIColor blackColor];
    label.font = [UIFont boldSystemFontOfSize:24.0];
    label.textAlignment = 1;
    [self.view addSubview:label];

    struct CGRect gameFrame;
    gameFrame.origin.x = 0;
    gameFrame.origin.y = 90;
    gameFrame.size.width = b.size.width;
    gameFrame.size.height = b.size.height - 90;
    GameView *game = [[GameView alloc] initWithFrame:gameFrame];
    [game setScoreLabel:label];
    [self.view addSubview:game];
}
@end

@interface AppDelegate : UIResponder
@property (nonatomic, assign) UIWindow *window;
@end

@implementation AppDelegate
- (BOOL)application:(UIApplication *)application didFinishLaunchingWithOptions:(id)options {
    struct CGRect bounds = [[UIScreen mainScreen] bounds];
    UIWindow *window = [[UIWindow alloc] initWithFrame:bounds];
    window.rootViewController = [[GameViewController alloc] init];
    [window makeKeyAndVisible];
    self.window = window;
    return YES;
}
@end

int main(int argc, char *argv[]) {
    return UIApplicationMain(argc, argv, nil, @"AppDelegate");
}
