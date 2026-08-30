//! 60-cell Express multilingual benchmark — server-side keyword inference acceptance.

use neuromesh_context::retrieval::{apply_auto_extract_keywords, infer_assisted_seed_signals};
use neuromesh_task::TaskSignatureExtractor;

struct MatrixCell {
    task: &'static str,
    prompt: &'static str,
    gold_keywords: &'static [&'static str],
}

fn matrix_cells() -> Vec<MatrixCell> {
    let tasks: [(&str, &[&str]); 6] = [
        ("routing", &["Router", "route", "app"]),
        ("middleware", &["app.use", "next", "middleware"]),
        ("render", &["res.render", "view", "render", "engine"]),
        ("static", &["express.static", "static", "stat"]),
        ("query", &["req.query", "query", "parseurl", "utils"]),
        ("session", &["cookie", "session", "cookie-session"]),
    ];

    let prompts: [(&str, [&str; 6]); 10] = [
        (
            "fa",
            [
                "اکسپرس چطور مسیریابی را انجام می‌دهد و مسیرها چگونه ثبت می‌شوند؟",
                "لوله‌ی میان‌افزارها (middleware pipeline) را توضیح بده و اینکه next() چطور کار می‌کند.",
                "تابع res.render() چطور با موتورهای قالب و ویوها کار می‌کند؟",
                "اکسپرس فایل‌های استاتیک را چگونه سرو می‌کند؟",
                "اکسپرس کوئری‌استرینگ درخواست را چطور به req.query تبدیل می‌کند؟",
                "کوکی‌ها و سشن‌ها در اکسپرس چطور کار می‌کنند؟",
            ],
        ),
        (
            "en",
            [
                "How does Express handle routing and how are routes registered?",
                "Explain the middleware pipeline and how next() works.",
                "How does res.render() work with template engines and views?",
                "How does Express serve static files?",
                "How does Express parse the request query string into req.query?",
                "How do cookies and sessions work in Express?",
            ],
        ),
        (
            "ar",
            [
                "كيف يقوم Express بإدارة التوجيه وكيف يتم تسجيل المسارات؟",
                "اشرح خط أنابيب الوسائط middleware وكيف تعمل الدالة next().",
                "كيف تعمل الدالة res.render() مع محركات القوالب وطرق العرض؟",
                "كيف يقدم Express الملفات الثابتة؟",
                "كيف يحول Express سلسلة الاستعلام إلى req.query؟",
                "كيف تعمل ملفات تعريف الارتباط والجلسات في Express؟",
            ],
        ),
        (
            "de",
            [
                "Wie steuert Express das Routing und wie werden Routen registriert?",
                "Erkläre die Middleware-Pipeline und wie next() funktioniert.",
                "Wie funktioniert res.render() mit Template-Engines und Views?",
                "Wie liefert Express statische Dateien aus?",
                "Wie parst Express den Query-String der Anfrage in req.query?",
                "Wie funktionieren Cookies und Sessions in Express?",
            ],
        ),
        (
            "fr",
            [
                "Comment Express gère-t-il le routage et comment les routes sont-elles enregistrées ?",
                "Explique le pipeline de middlewares et comment fonctionne next().",
                "Comment res.render() fonctionne-t-il avec les moteurs de templates et les vues ?",
                "Comment Express sert-il les fichiers statiques ?",
                "Comment Express analyse-t-il la chaîne de requête dans req.query ?",
                "Comment fonctionnent les cookies et les sessions dans Express ?",
            ],
        ),
        (
            "es",
            [
                "¿Cómo maneja Express el enrutamiento y cómo se registran las rutas?",
                "Explica la cadena de middlewares y cómo funciona next().",
                "¿Cómo funciona res.render() con los motores de plantillas y vistas?",
                "¿Cómo sirve Express los archivos estáticos?",
                "¿Cómo analiza Express la cadena de consulta en req.query?",
                "¿Cómo funcionan las cookies y las sesiones en Express?",
            ],
        ),
        (
            "zh",
            [
                "Express 如何处理路由以及路由是如何注册的？",
                "解释中间件管道以及 next() 如何工作。",
                "res.render() 如何与模板引擎和视图一起工作？",
                "Express 如何提供静态文件？",
                "Express 如何将请求查询字符串解析为 req.query？",
                "Express 中的 Cookie 和会话是如何工作的？",
            ],
        ),
        (
            "ja",
            [
                "Expressはルーティングをどのように処理し、ルートはどのように登録されますか？",
                "ミドルウェアパイプラインとnext()の仕組みを説明してください。",
                "res.render()はテンプレートエンジンやビューとどのように連携しますか？",
                "Expressは静的ファイルをどのように配信しますか？",
                "Expressはリクエストのクエリ文字列をどのようにreq.queryに解析しますか？",
                "ExpressでCookieとセッションはどのように機能しますか？",
            ],
        ),
        (
            "ru",
            [
                "Как Express обрабатывает маршрутизацию и как регистрируются маршруты?",
                "Объясни конвейер промежуточных обработчиков и как работает next().",
                "Как res.render() работает с шаблонизаторами и представлениями?",
                "Как Express раздаёт статические файлы?",
                "Как Express преобразует строку запроса в req.query?",
                "Как работают куки и сессии в Express?",
            ],
        ),
        (
            "tr",
            [
                "Express yönlendirmeyi nasıl yönetiyor ve rotalar nasıl kaydediliyor?",
                "Ara katman (middleware) zincirini ve next()'in nasıl çalıştığını açıkla.",
                "res.render() şablon motorları ve görünümlerle nasıl çalışır?",
                "Express statik dosyaları nasıl sunar?",
                "Express istek sorgu dizesini req.query'e nasıl ayrıştırır?",
                "Express'te çerezler ve oturumlar nasıl çalışır?",
            ],
        ),
    ];

    let task_names = [
        "routing",
        "middleware",
        "render",
        "static",
        "query",
        "session",
    ];
    let mut out = Vec::with_capacity(60);
    for (lang, lang_prompts) in prompts {
        for (ti, prompt) in lang_prompts.iter().enumerate() {
            out.push(MatrixCell {
                task: task_names[ti],
                prompt,
                gold_keywords: tasks[ti].1,
            });
        }
        let _ = lang;
    }
    out
}

fn gold_hits(keywords: &[String], gold: &[&str]) -> usize {
    gold.iter()
        .filter(|g| keywords.iter().any(|k| k.eq_ignore_ascii_case(g)))
        .count()
}

#[test]
fn express_matrix_infer_at_least_two_gold_keyword_hits() {
    let cells = matrix_cells();
    assert_eq!(cells.len(), 60);
    let mut failures = Vec::new();
    for cell in &cells {
        let (kw, exp) = infer_assisted_seed_signals(cell.prompt);
        let hits = gold_hits(&kw, cell.gold_keywords);
        if hits < 2 || exp.is_empty() {
            failures.push(format!("{}: hits={hits} kw={kw:?} exp={exp:?}", cell.task));
        }
    }
    assert!(
        failures.is_empty(),
        "infer failed on {} cells:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn partial_fill_keywords_only_adds_expansion() {
    let prompt = "Explain the middleware pipeline and how next() works.";
    let mut sig = TaskSignatureExtractor::extract(prompt);
    sig.client_keywords = vec!["app.use".into()];
    apply_auto_extract_keywords(&mut sig, prompt, true);
    assert_eq!(sig.client_keywords, vec!["app.use"]);
    assert!(!sig.client_expansion.is_empty());
}

#[test]
fn partial_fill_expansion_only_adds_keywords() {
    let prompt = "Explain the middleware pipeline and how next() works.";
    let mut sig = TaskSignatureExtractor::extract(prompt);
    sig.client_expansion = vec!["pipeline".into()];
    apply_auto_extract_keywords(&mut sig, prompt, true);
    assert!(sig.client_keywords.iter().any(|k| k == "next"));
    assert_eq!(sig.client_expansion, vec!["pipeline"]);
}

#[test]
fn client_both_sides_prevents_duplicates_only() {
    let prompt = "Explain middleware pipeline";
    let mut sig = TaskSignatureExtractor::extract(prompt);
    sig.client_keywords = vec!["app.use".into(), "next".into(), "middleware".into()];
    sig.client_expansion = vec!["middleware".into(), "pipeline".into(), "route".into()];
    apply_auto_extract_keywords(&mut sig, prompt, true);
    assert_eq!(sig.client_keywords.len(), 3);
    assert_eq!(sig.client_expansion.len(), 3);
}

#[test]
fn auto_extract_disabled_leaves_empty() {
    let prompt = "Explain middleware pipeline and next()";
    let mut sig = TaskSignatureExtractor::extract(prompt);
    apply_auto_extract_keywords(&mut sig, prompt, false);
    assert!(sig.client_keywords.is_empty());
    assert!(sig.client_expansion.is_empty());
}
