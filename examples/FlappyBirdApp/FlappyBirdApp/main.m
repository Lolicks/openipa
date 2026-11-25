// FlappyBirdApp — игра с software-3D фоном, liquid-glass HUD и бустами.
// Реальный UIKit + CoreGraphics; никаких заглушек фреймворков.

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
void CGContextSetAlpha(CGContextRef context, double alpha);
void CGContextMoveToPoint(CGContextRef context, double x, double y);
void CGContextAddLineToPoint(CGContextRef context, double x, double y);
void CGContextClosePath(CGContextRef context);
void CGContextFillPath(CGContextRef context);

// Собственная аппроксимация синуса — без внешних libm-символов.
static double oi_sin(double x) {
    const double two_pi = 6.283185307179586;
    const double pi = 3.141592653589793;
    while (x > pi) x -= two_pi;
    while (x < -pi) x += two_pi;
    double term = x, sum = x, x2 = x * x;
    for (int i = 1; i <= 7; i++) {
        term *= -x2 / ((double)(2 * i) * (double)(2 * i + 1));
        sum += term;
    }
    return sum;
}
static double oi_cos(double x) {
    return oi_sin(x + 1.5707963267948966);
}

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
+ (UIColor *)clearColor;
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
@property (nonatomic, assign) double alpha;
@end

@interface UIBlurEffect : NSObject
+ (UIBlurEffect *)effectWithStyle:(long)style;
@end

@interface UIVisualEffectView : UIView
- (instancetype)initWithEffect:(id)effect;
@property (nonatomic, readonly) UIView *contentView;
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
    double coinX;
    double coinY;
    double angle;
    int score;
    int coins;
    int shield;
    int gameOver;
    int tickCount;
    double particleX[12];
    double particleY[12];
    double particleVX[12];
    double particleVY[12];
    int particleLife[12];
    UILabel *scoreLabel;
    UILabel *hintLabel;
}
- (void)setScoreLabel:(UILabel *)score hint:(UILabel *)hint;
@end

@implementation GameView
- (instancetype)initWithFrame:(struct CGRect)frame {
    self = [super initWithFrame:frame];
    birdY = frame.size.height / 2;
    velocity = 0;
    pipeX = frame.size.width;
    pipeTop = 120;
    coinX = frame.size.width + 120;
    coinY = 180;
    angle = 0;
    score = 0;
    coins = 0;
    shield = 0;
    gameOver = 0;
    tickCount = 0;
    for (int i = 0; i < 12; i++) particleLife[i] = 0;
    [NSTimer scheduledTimerWithTimeInterval:0.02 target:self selector:@selector(tick) userInfo:nil repeats:YES];
    return self;
}

- (void)setScoreLabel:(UILabel *)scoreView hint:(UILabel *)hintView {
    scoreLabel = scoreView;
    hintLabel = hintView;
    [scoreLabel setText:@"Score: 0  |  Coins: 0"];
    [hintLabel setText:@"TAP TO FLAP"];
}

- (void)burstAtX:(double)x y:(double)y {
    for (int i = 0; i < 12; i++) {
        particleX[i] = x;
        particleY[i] = y;
        particleVX[i] = ((i * 17) % 11 - 5) * 0.6;
        particleVY[i] = ((i * 13) % 9 - 4) * 0.6;
        particleLife[i] = 30 + (i % 10);
    }
}

- (void)tick {
    struct CGRect bounds = self.bounds;
    tickCount++;
    angle += 0.03;
    if (!gameOver) {
        velocity += 0.30;
        birdY += velocity;
        pipeX -= 3.0;
        coinX -= 3.0;
        if (coinX < -30) {
            coinX = bounds.size.width + 60 + ((tickCount * 29) % 120);
            coinY = 80 + ((tickCount * 53) % 240);
        }
        if (pipeX < -60) {
            pipeX = bounds.size.width + 20;
            pipeTop = 70 + ((tickCount * 37) % 150);
            score++;
        }
        if (coinX > 50 && coinX < 110 && coinY > birdY - 10 && coinY < birdY + 50) {
            coinX = -100;
            coins++;
            if (coins % 5 == 0) {
                shield = 1;
                [hintLabel setText:@"SHIELD ON"];
            } else {
                [hintLabel setText:@"+1 COIN"];
            }
            [self burstAtX:coinX y:coinY];
        }
        double birdLeft = 60, birdRight = 100;
        double gapBottom = pipeTop + 150;
        int hit = 0;
        if (birdY > bounds.size.height - 40 || birdY < 0) hit = 1;
        if (birdRight > pipeX && birdLeft < pipeX + 60) {
            if (birdY < pipeTop || birdY + 30 > gapBottom) hit = 1;
        }
        if (hit) {
            if (shield) {
                shield = 0;
                [hintLabel setText:@"SHIELD SAVED YOU"];
                birdY = bounds.size.height / 2;
                velocity = 0;
                [self burstAtX:80 y:birdY];
            } else {
                gameOver = 1;
                [hintLabel setText:@"TAP TO RESTART"];
                [self burstAtX:80 y:birdY];
            }
        }
        [scoreLabel setText:[NSString stringWithFormat:@"Score: %d  |  Coins: %d", score, coins]];
    }
    for (int i = 0; i < 12; i++) {
        if (particleLife[i] > 0) {
            particleX[i] += particleVX[i];
            particleY[i] += particleVY[i];
            particleVY[i] += 0.08;
            particleLife[i]--;
        }
    }
    [self setNeedsDisplay];
}

- (void)touchesBegan:(id)touches withEvent:(id)event {
    if (gameOver) {
        birdY = self.bounds.size.height / 2;
        velocity = 0;
        pipeX = self.bounds.size.width;
        coinX = self.bounds.size.width + 120;
        score = 0;
        gameOver = 0;
        [hintLabel setText:@"TAP TO FLAP"];
    } else {
        velocity = -6.0;
    }
}

- (void)drawCubeInContext:(CGContextRef)context centerX:(double)cx centerY:(double)cy {
    // 8 вершин куба и 6 граней с software-проекцией и alpha-смешиванием.
    double vx[8], vy[8], vz[8];
    double base[8][3] = {
        {-60,-60,-60}, {60,-60,-60}, {60,60,-60}, {-60,60,-60},
        {-60,-60,60}, {60,-60,60}, {60,60,60}, {-60,60,60}
    };
    double ca = oi_cos(angle), sa = oi_sin(angle);
    double cb = oi_cos(angle * 0.5), sb = oi_sin(angle * 0.5);
    for (int i = 0; i < 8; i++) {
        double x = base[i][0], y = base[i][1], z = base[i][2];
        double x1 = x * ca + z * sa;
        double z1 = -x * sa + z * ca;
        double y2 = y * cb - z1 * sb;
        double z2 = y * sb + z1 * cb;
        double scale = 260.0 / (260.0 + z2);
        vx[i] = cx + x1 * scale;
        vy[i] = cy + y2 * scale;
        vz[i] = z2;
    }
    int faces[6][4] = {
        {0,1,2,3}, {4,5,6,7}, {0,1,5,4}, {2,3,7,6}, {1,2,6,5}, {0,3,7,4}
    };
    CGContextSetRGBFillColor(context, 0.35, 0.75, 1.0, 1.0);
    for (int f = 0; f < 6; f++) {
        CGContextSetAlpha(context, 0.10 + (f % 3) * 0.04);
        CGContextMoveToPoint(context, vx[faces[f][0]], vy[faces[f][0]]);
        for (int k = 1; k < 4; k++) {
            CGContextAddLineToPoint(context, vx[faces[f][k]], vy[faces[f][k]]);
        }
        CGContextClosePath(context);
        CGContextFillPath(context);
    }
    CGContextSetAlpha(context, 1.0);
}

- (void)drawRect:(struct CGRect)rect {
    CGContextRef context = UIGraphicsGetCurrentContext();
    UIColor *sky = [UIColor colorWithRed:0.53 green:0.81 blue:0.92 alpha:1.0];
    UIColor *pipeColor = [UIColor colorWithRed:0.13 green:0.55 blue:0.13 alpha:1.0];
    UIColor *birdColor = [UIColor colorWithRed:1.0 green:0.75 blue:0.1 alpha:1.0];
    UIColor *coinColor = [UIColor colorWithRed:1.0 green:0.9 blue:0.2 alpha:1.0];

    CGContextSetFillColorWithColor(context, [sky CGColor]);
    CGContextFillRect(context, rect);

    // 3D-фон.
    [self drawCubeInContext:context centerX:rect.size.width / 2 centerY:rect.size.height / 2];

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

    // Монета.
    struct CGRect coin;
    coin.origin.x = coinX;
    coin.origin.y = coinY;
    coin.size.width = 22;
    coin.size.height = 22;
    CGContextSetFillColorWithColor(context, [coinColor CGColor]);
    CGContextFillEllipseInRect(context, coin);

    // Частицы.
    CGContextSetRGBFillColor(context, 1.0, 1.0, 1.0, 1.0);
    for (int i = 0; i < 12; i++) {
        if (particleLife[i] > 0) {
            struct CGRect p;
            p.origin.x = particleX[i];
            p.origin.y = particleY[i];
            p.size.width = 5;
            p.size.height = 5;
            CGContextFillRect(context, p);
        }
    }

    // Птица.
    CGContextSetFillColorWithColor(context, [birdColor CGColor]);
    struct CGRect bird;
    bird.origin.x = 60;
    bird.origin.y = birdY;
    bird.size.width = 40;
    bird.size.height = 30;
    CGContextFillEllipseInRect(context, bird);
    if (shield) {
        CGContextSetAlpha(context, 0.5);
        struct CGRect halo;
        halo.origin.x = 52;
        halo.origin.y = birdY - 8;
        halo.size.width = 56;
        halo.size.height = 46;
        CGContextFillEllipseInRect(context, halo);
        CGContextSetAlpha(context, 1.0);
    }

    CGContextSetRGBFillColor(context, 0.6, 0.45, 0.25, 1.0);
    struct CGRect ground;
    ground.origin.x = 0;
    ground.origin.y = rect.size.height - 40;
    ground.size.width = rect.size.width;
    ground.size.height = 40;
    CGContextFillRect(context, ground);
}
@end

@interface GameViewController : UIViewController
@end

@implementation GameViewController
- (void)viewDidLoad {
    [super viewDidLoad];
    self.view.backgroundColor = [UIColor whiteColor];
    struct CGRect b = self.view.bounds;

    // Liquid-glass HUD: размытие + полупрозрачная панель.
    UIBlurEffect *blur = [UIBlurEffect effectWithStyle:2];
    UIVisualEffectView *glass = [[UIVisualEffectView alloc] initWithEffect:blur];
    struct CGRect glassFrame;
    glassFrame.origin.x = 12;
    glassFrame.origin.y = 30;
    glassFrame.size.width = b.size.width - 24;
    glassFrame.size.height = 100;
    glass.frame = glassFrame;
    glass.alpha = 0.92;
    [self.view addSubview:glass];

    struct CGRect scoreFrame;
    scoreFrame.origin.x = 0;
    scoreFrame.origin.y = 8;
    scoreFrame.size.width = b.size.width - 24;
    scoreFrame.size.height = 40;
    UILabel *score = [[UILabel alloc] initWithFrame:scoreFrame];
    score.text = @"Score: 0  |  Coins: 0";
    score.textColor = [UIColor blackColor];
    score.font = [UIFont boldSystemFontOfSize:22.0];
    score.textAlignment = 1;
    [[glass contentView] addSubview:score];

    struct CGRect hintFrame;
    hintFrame.origin.x = 0;
    hintFrame.origin.y = 50;
    hintFrame.size.width = b.size.width - 24;
    hintFrame.size.height = 34;
    UILabel *hint = [[UILabel alloc] initWithFrame:hintFrame];
    hint.text = @"TAP TO FLAP";
    hint.textColor = [UIColor blackColor];
    hint.font = [UIFont boldSystemFontOfSize:16.0];
    hint.textAlignment = 1;
    [[glass contentView] addSubview:hint];

    struct CGRect gameFrame;
    gameFrame.origin.x = 0;
    gameFrame.origin.y = 140;
    gameFrame.size.width = b.size.width;
    gameFrame.size.height = b.size.height - 140;
    GameView *game = [[GameView alloc] initWithFrame:gameFrame];
    [game setScoreLabel:score hint:hint];
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
