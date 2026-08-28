/* NeuroMesh landing — EN (default) + FA (RTL) */
window.NMI18n = (function () {
  const STORAGE_KEY = 'neuromesh-lang';

  const dict = {
    en: {
      'meta.title': "NeuroMesh — Don't delete the extra code. Fold it.",
      'meta.description': 'NeuroMesh: a neural graph in RAM, reversible one-line folds, and an evidence packet instead of thousand-line files dumped into your AI editor.',

      'nav.pain': 'The pain',
      'nav.fold': 'Fold',
      'nav.galaxy': 'Galaxy',
      'nav.learning': 'Learning',
      'nav.install': 'Install',
      'nav.connect': 'Connect',
      'nav.tools': 'Tools',
      'nav.github': 'GitHub',
      'nav.installBtn': 'Install',

      'hero.slimeTag': 'Physarum routing · live neural mesh',
      'hero.badgeRelease': 'Latest release',
      'hero.desc': 'How does nature pack two metres of DNA into a nucleus without deleting a single letter? Not by throwing genes away — by <strong>folding</strong>. NeuroMesh does the same to your repository: a neural graph in RAM, reversible one-line folds, and an evidence packet instead of three thousand-line files dumped into Cursor or Claude.',
      'hero.ctaInstall': 'Quick install',
      'hero.ctaDocs': 'Documentation',
      'hero.ctaStar': 'Star on GitHub',
      'hero.clients': 'Local-first MCP ·',
      'hero.clientsList': 'Cursor · Codex · OpenCode · MiMo CLI · Antigravity · VS Code · Claude · Kilo · Trae · Windsurf · Zed',
      'hero.foldHeader': 'src/handler.rs — folded skeleton',

      'panel.graph': 'Graph · RAM mesh',
      'panel.code': 'Code · fold splice',
      'panel.engine': 'Engine · packet',

      'pain.label': 'The problem',
      'pain.title': 'The pain',
      'pain.lead': 'You ask a simple question in a large project. The editor copies two or three <strong>thousand-line files</strong> and ships them to the model.',
      'pain.c1.title': 'Tokens you never needed',
      'pain.c1.desc': 'Dollar cost on every turn — paying for helpers you will never touch.',
      'pain.c2.title': 'Seconds of fake loading',
      'pain.c2.desc': 'While the window fills with unrelated bodies and private utilities.',
      'pain.c3.title': 'Lost in the middle',
      'pain.c3.desc': "The model drowns in unrelated code and invents bugs that don't exist.",
      'pain.th.approach': 'Approach',
      'pain.th.wrong': 'What goes wrong',
      'pain.r1.a': 'Vector RAG',
      'pain.r1.b': 'Chunks smash functions. The shape of the code disappears.',
      'pain.r2.a': '"Just attach the files"',
      'pain.r2.b': 'The model sees everything and understands nothing.',
      'pain.r3.a': 'A static code graph',
      'pain.r3.b': 'Better map — then it still pastes <strong>full files</strong> into the prompt.',
      'pain.r4.a': 'NeuroMesh',
      'pain.r4.b': '<strong>Route first, then fold.</strong> The graph finds the path. The packet is what the model reads.',

      'fold.label': 'Core concept',
      'fold.title': "Don't delete. Fold.",
      'fold.lead': 'Nature does not delete DNA to fit a nucleus. It <strong>supercoils</strong>. NeuroMesh treats the syntax tree like a genetic strand in RAM.',
      'fold.c1.title': 'Exons — expressed',
      'fold.c1.desc': 'Functions you need stay fully visible — real body, real lines.',
      'fold.c2.title': 'Introns — folded',
      'fold.c2.desc': 'Everything else collapses to a one-line reversible marker with a fold ID.',
      'fold.c3.title': 'Wake on demand',
      'fold.c3.desc': '<code style="color:var(--purple-fg)">neuromesh_expand_fold</code> unsplices from memory. No disk grep.',
      'fold.quote': '<strong>Structure stays. Tokens sleep. Wake a fold when you need it.</strong>',
      'fold.bio.label': 'Biomimicry',
      'fold.bio.title': 'Inspired by living systems',

      'nature.n1.t': 'DNA supercoiling',
      'nature.n1.e': 'Genetic skeletonizer',
      'nature.n1.p': 'Fold unused bodies; keep the map of the file.',
      'nature.n2.t': 'Physarum (slime mold)',
      'nature.n2.e': 'Steiner / shortest tissue',
      'nature.n2.p': 'Grow the cheapest path between seeds — not the whole neighborhood.',
      'nature.n3.t': 'Synapses & STDP',
      'nature.n3.e': 'Pheromone edges + record_feedback',
      'nature.n3.p': 'Paths you actually edited get stronger next time.',
      'nature.n4.t': 'Cell membrane',
      'nature.n4.e': 'QualityGate',
      'nature.n4.p': 'Tight by default; auth / payment tasks open the membrane.',
      'nature.n5.t': 'Mycelium',
      'nature.n5.e': 'Hyphal prefetch',
      'nature.n5.p': 'Warm the next hop before the second tool call.',
      'nature.n6.t': 'Neural mesh',
      'nature.n6.e': 'Project graph in RAM',
      'nature.n6.p': 'Files, functions, Imports, Calls — a nervous system, not a bag of strings.',

      'galaxy.label': 'Visualization',
      'galaxy.title': '3D Neural Galaxy',
      'galaxy.lead': '<code style="color:var(--accent-fg)">neuromesh monitor</code> is the live mesh: subsystems as a constellation, then the file graph, then the symbols inside a module.',
      'galaxy.c1': 'Constellation — crates and subsystems',
      'galaxy.c2': '3D galaxy — files and synapses; Physarum tubes',
      'galaxy.c3': 'Module zoom — files and AST symbols in one crate',
      'galaxy.url': 'Default URL:',
      'galaxy.port': 'Port:',

      'how.label': 'Pipeline',
      'how.title': 'How a turn actually goes',
      'how.lead': 'Route first, then fold. Seeds are never truncated to fake a small packet.',
      'how.phase1.title': 'Read',
      'how.phase1.sub': 'Task as written',
      'how.phase2.title': 'Route',
      'how.phase2.sub': 'Graph + Physarum',
      'how.phase3.title': 'Splice',
      'how.phase3.sub': 'Fold + packet',
      'how.detail.default': 'Read the task exactly as written — intent is not lowercased into mush.',
      'how.stepsOf': 'of 8 steps',
      'how.pauseTour': '⏸ Pause tour',
      'how.playTour': '▶ Play tour',
      'how.mode1': 'Tiny, obvious edits',
      'how.mode2': 'Everyday development',
      'how.mode3': 'Refactors, auth, critical paths',
      'how.extraTokens': 'extra tokens',
      'how.tagDefault': 'Default',

      'learn.label': 'Synaptic STDP',
      'learn.title': 'Smarter every turn',
      'learn.lead': "NeuroMesh doesn't forget what worked. Every successful edit is experience — <code>neuromesh_record_feedback</code> strengthens the synapses on the path you actually used. Next packet, the mesh routes faster and closer to gold.",
      'learn.turn': 'Turn',
      'learn.turnOf': '/ 3',
      'learn.turnCap1': 'First visit — the mesh explores weaker paths.',
      'learn.loopTitle': 'The learning loop',
      'learn.loop1.t': 'get_context',
      'learn.loop1.s': 'Route on pheromone edges',
      'learn.loop2.t': 'Edit & succeed',
      'learn.loop2.s': 'Agent touches real files',
      'learn.loop3.t': 'record_feedback',
      'learn.loop3.s': 'Spike synapses — STDP learns',
      'learn.loop4.t': 'Next packet',
      'learn.loop4.s': 'Stronger path, fewer misses',
      'learn.m1': 'Synaptic strength',
      'learn.m2': 'Recall',
      'learn.m3': 'Grep fallbacks',
      'learn.c1.t': 'Experience from edits',
      'learn.c1.p': 'Only paths the agent <em>actually edited</em> get credit. No fake links from guessing — unique resolve keeps the graph honest.',
      'learn.c2.t': 'Pheromone edges',
      'learn.c2.p': 'Co-edited files share stronger tubes. Physarum prefers reinforced routes — like slime mold remembering where food was.',
      'learn.c3.t': 'Mycelial memory',
      'learn.c3.p': "Hyphal prefetch warms the next hop in RAM. The mesh doesn't just learn routes — it pre-warms what you'll expand next.",
      'learn.quote': '<strong>Fire together, wire together.</strong> Skip <code>record_feedback</code> and the next packet starts from zero plasticity.',

      'stats.label': 'Measured',
      'stats.title': 'What we actually measured',
      'stats.lead': 'Savings are per task, after folding. Re-run: <code>neuromesh eval</code>',
      'stats.sub': 'Release 2026-08-28 v0.7.17 · 650,859 workspace tokens',
      'stats.s1': '% vs workspace (handle_tool_call)',
      'stats.s2': '% vs workspace (physarum)',
      'stats.s3': 'Files indexed',
      'stats.s4': 'ms packet activation',

      'install.label': 'Get started',
      'install.title': 'Install',
      'install.lead': 'One command. Then <code>neuromesh doctor</code> and <code>neuromesh connect</code>.',
      'install.tabMac': 'macOS / Linux',
      'install.tabWin': 'Windows',
      'install.tabSource': 'From source',
      'install.tabCargo': 'Cargo',
      'install.after': 'After install',
      'install.terminal': 'Terminal',
      'install.powershell': 'PowerShell',
      'install.buildSource': 'Build from source (rustup 1.80+)',
      'install.cargoInstall': 'Cargo install',
      'connect.snippet': 'MCP config snippet',

      'connect.label': 'Integration',
      'connect.title': 'Connect',
      'connect.lead': 'Native <strong>MCP stdio</strong> — Cursor, Claude, Codex, OpenCode, MiMo CLI, Antigravity, VS Code, Kilo Code, Trae, Windsurf, Cline/Roo, Zed, and more. <code>neuromesh connect</code> writes an absolute binary path so the agent doesn\'t need NeuroMesh on PATH.',
      'connect.note': 'Run <code>neuromesh connect</code> to write configs for installed apps, or <code>neuromesh connect --print</code> for snippets.',

      'tools.label': 'MCP Tools',
      'tools.title': 'Tools',
      'tools.lead': 'Tell the agent: get context → expand fold if needed → trace → record feedback.',
      'tools.footer': 'Rust, TypeScript, Python, Go, Java, Kotlin, PHP, C#, Dart, Swift, Ruby — tree-sitter queries. Framework overlays for Laravel, Django, Next, Nuxt, Spring, Android, and 30+ more.',

      'docs.label': 'Learn more',
      'docs.title': 'Documentation',
      'docs.d1.t': 'Living systems',
      'docs.d1.s': 'DNA, Physarum, STDP — mapped to crates',
      'docs.d2.t': 'Architecture',
      'docs.d2.s': 'Pipeline and guarantees',
      'docs.d3.t': 'MCP & CLI',
      'docs.d3.s': 'Tools and commands reference',
      'docs.d4.t': 'Agent guide',
      'docs.d4.s': 'Per-IDE setup tutorial',
      'docs.d5.t': 'Quality',
      'docs.d5.s': 'Gold, eval, numbers',
      'docs.d6.t': 'Contributing',
      'docs.d6.s': 'Build a solver or a language',

      'footer.tagline': "NeuroMesh · Don't delete the extra code. Fold it.",
      'footer.changelog': 'Changelog',

      'ui.copy': 'Copy',
      'ui.copied': 'Copied!',
      'ui.failed': 'Failed',
      'ui.lightbox': 'Image preview',
      'ui.close': 'Close',
      'ui.prev': 'Previous',
      'ui.next': 'Next',
      'ui.lang': 'Language',
      'ui.langEn': 'English',
      'ui.langFa': 'فارسی',
    },
    fa: {
      'meta.title': 'NeuroMesh — کد اضافه را حذف نکن؛ تا کن.',
      'meta.description': 'NeuroMesh: گراف عصبی در RAM، تا کردن یک‌خطی و برگشت‌پذیر، و بستهٔ شواهد — به‌جای ریختن هزاران خط کد در Cursor یا Claude.',

      'nav.pain': 'مشکل',
      'nav.fold': 'تا کردن',
      'nav.galaxy': 'کهکشان',
      'nav.learning': 'یادگیری',
      'nav.install': 'نصب',
      'nav.connect': 'اتصال',
      'nav.tools': 'ابزارها',
      'nav.github': 'GitHub',
      'nav.installBtn': 'نصب',

      'hero.slimeTag': 'مسیریابی Physarum · مش عصبی زنده',
      'hero.badgeRelease': 'آخرین انتشار',
      'hero.desc': 'طبیعت دو متر DNA را چطور در هسته جا می‌دهد، بدون اینکه یک حرف حذف کند؟ با <strong>تا کردن</strong>، نه دور انداختن. NeuroMesh همین کار را با مخزن کد شما می‌کند: گراف عصبی در RAM، تا کردن یک‌خطی و برگشت‌پذیر، و بستهٔ شواهد — به‌جای سه هزار خط فایل در Cursor یا Claude.',
      'hero.ctaInstall': 'نصب سریع',
      'hero.ctaDocs': 'مستندات',
      'hero.ctaStar': 'ستاره در GitHub',
      'hero.clients': 'MCP محلی ·',
      'hero.clientsList': 'Cursor · Codex · OpenCode · MiMo CLI · Antigravity · VS Code · Claude · Kilo · Trae · Windsurf · Zed',
      'hero.foldHeader': 'src/handler.rs — اسکلت تا شده',

      'panel.graph': 'گراف · مش RAM',
      'panel.code': 'کد · برش تا',
      'panel.engine': 'موتور · بسته',

      'pain.label': 'مسئله',
      'pain.title': 'درد واقعی کار با AI',
      'pain.lead': 'در پروژهٔ بزرگ یک سؤال ساده می‌پرسید. ویرایشگر دو یا سه <strong>فایل هزارخطی</strong> را کپی می‌کند و مستقیم برای مدل می‌فرستد.',
      'pain.c1.title': 'توکن‌های بی‌دلیل',
      'pain.c1.desc': 'هزینهٔ دلاری در هر نوبت — برای توابع کمکی که هرگز به آن‌ها نمی‌رسید.',
      'pain.c2.title': 'انتظارِ توخالی',
      'pain.c2.desc': 'چند ثانیه صبر می‌کنید تا پنجره از بدنه‌های نامرتبط و utilityهای خصوصی پر شود.',
      'pain.c3.title': 'غرق شدن در میانه',
      'pain.c3.desc': 'مدل در کد نامرتبط غرق می‌شود و باگ‌هایی می‌سازد که اصلاً وجود ندارند.',
      'pain.th.approach': 'رویکرد',
      'pain.th.wrong': 'اشکال کجاست',
      'pain.r1.a': 'Vector RAG',
      'pain.r1.b': 'تکه‌تکه کردن، ساختار تابع را از بین می‌برد و نقشهٔ کد محو می‌شود.',
      'pain.r2.a': '«فقط فایل‌ها را attach کن»',
      'pain.r2.b': 'مدل همه‌چیز را می‌بیند، اما در نهایت هیچ‌چیز را درست نمی‌فهمد.',
      'pain.r3.a': 'گراف کد ایستا',
      'pain.r3.b': 'نقشه بهتر است — اما باز هم <strong>فایل کامل</strong> را در prompt می‌چسباند.',
      'pain.r4.a': 'NeuroMesh',
      'pain.r4.b': '<strong>اول مسیریابی، بعد تا.</strong> گراف مسیر را پیدا می‌کند؛ بسته همان چیزی است که مدل می‌خواند.',

      'fold.label': 'ایدهٔ اصلی',
      'fold.title': 'حذف نکن. تا کن.',
      'fold.lead': 'طبیعت DNA را برای جا شدن در هسته حذف نمی‌کند — آن را <strong>ابرمی‌پیچد</strong>. NeuroMesh درخت نحوی را مثل رشتهٔ ژنتیک در RAM می‌بیند.',
      'fold.c1.title': 'Exon — بخشِ بیان‌شده',
      'fold.c1.desc': 'تابع‌های مورد نیاز کاملاً باز می‌مانند: بدنه و خط واقعی.',
      'fold.c2.title': 'Intron — بخشِ تا شده',
      'fold.c2.desc': 'بقیه به یک نشانگر یک‌خطی و برگشت‌پذیر با fold ID جمع می‌شوند.',
      'fold.c3.title': 'بیدار کردن درخواستی',
      'fold.c3.desc': '<code style="color:var(--purple-fg)">neuromesh_expand_fold</code> از حافظه باز می‌کند — بدون grep روی دیسک.',
      'fold.quote': '<strong>ساختار می‌ماند. توکن‌ها می‌خوابند. وقتی لازم شد، fold را بیدار کن.</strong>',
      'fold.bio.label': 'تقلید از طبیعت',
      'fold.bio.title': 'الهام از سیستم‌های زنده',

      'nature.n1.t': 'ابرمی‌پیچی DNA',
      'nature.n1.e': 'Genetic skeletonizer',
      'nature.n1.p': 'بدنه‌های بلااستفاده تا می‌شوند؛ نقشهٔ فایل حفظ می‌ماند.',
      'nature.n2.t': 'Physarum (کپک مخاطی)',
      'nature.n2.e': 'Steiner / کوتاه‌ترین بافت',
      'nature.n2.p': 'کم‌هزینه‌ترین مسیر بین seedها — نه کل همسایگی.',
      'nature.n3.t': 'سیناپس و STDP',
      'nature.n3.e': 'لبهٔ فرومون + record_feedback',
      'nature.n3.p': 'مسیرهایی که واقعاً ویرایش شدند، بار بعد قوی‌تر می‌شوند.',
      'nature.n4.t': 'غشای سلول',
      'nature.n4.e': 'QualityGate',
      'nature.n4.p': 'پیش‌فرض سخت‌گیرانه؛ taskهای auth/payment غشا را باز می‌کنند.',
      'nature.n5.t': 'Mycelium',
      'nature.n5.e': 'Hyphal prefetch',
      'nature.n5.p': 'پرش بعدی را قبل از tool call دوم گرم می‌کند.',
      'nature.n6.t': 'مش عصبی',
      'nature.n6.e': 'گراف پروژه در RAM',
      'nature.n6.p': 'فایل، تابع، Imports، Calls — سیستم عصبی، نه کیسهٔ رشته.',

      'galaxy.label': 'نمایش',
      'galaxy.title': 'کهکشان عصبی سه‌بعدی',
      'galaxy.lead': '<code style="color:var(--accent-fg)">neuromesh monitor</code> مش زنده است: subsystemها مثل صورت‌فلکی، سپس گراف فایل، و در نهایت symbolهای داخل ماژول.',
      'galaxy.c1': 'صورت‌فلکی — crate و subsystem',
      'galaxy.c2': 'کهکشان 3D — فایل و سیناپس؛ لوله‌های Physarum',
      'galaxy.c3': 'زوم ماژول — فایل و symbolهای AST در یک crate',
      'galaxy.url': 'URL پیش‌فرض:',
      'galaxy.port': 'پورت:',

      'how.label': 'Pipeline',
      'how.title': 'یک نوبت واقعاً چطور پیش می‌رود',
      'how.lead': 'اول مسیریابی، بعد تا. seedها هرگز برای کوچک‌نمایی بریده نمی‌شوند.',
      'how.phase1.title': 'خواندن',
      'how.phase1.sub': 'Task همان‌طور که نوشته شده',
      'how.phase2.title': 'مسیریابی',
      'how.phase2.sub': 'Graph + Physarum',
      'how.phase3.title': 'برش',
      'how.phase3.sub': 'Fold + بسته',
      'how.detail.default': 'Task دقیقاً همان‌طور خوانده می‌شود — intent خرد نمی‌شود.',
      'how.stepsOf': 'از ۸ مرحله',
      'how.pauseTour': '⏸ توقف تور',
      'how.playTour': '▶ پخش تور',
      'how.mode1': 'ویرایش‌های کوچک و واضح',
      'how.mode2': 'توسعهٔ روزمره',
      'how.mode3': 'Refactor، auth، مسیرهای بحرانی',
      'how.extraTokens': 'توکن اضافه',
      'how.tagDefault': 'پیش‌فرض',

      'learn.label': 'STDP سیناپسی',
      'learn.title': 'هر نوبت باهوش‌تر',
      'learn.lead': 'NeuroMesh آنچه جواب داده را فراموش نمی‌کند. هر ویرایش موفق، یک تجربه است — <code>neuromesh_record_feedback</code> سیناپس‌های مسیر واقعی را تقویت می‌کند. بستهٔ بعدی سریع‌تر و نزدیک‌تر به recall طلایی مسیریابی می‌شود.',
      'learn.turn': 'نوبت',
      'learn.turnOf': '/ ۳',
      'learn.turnCap1': 'اولین بازدید — مش مسیرهای ضعیف‌تر را کاوش می‌کند.',
      'learn.loopTitle': 'حلقهٔ یادگیری',
      'learn.loop1.t': 'get_context',
      'learn.loop1.s': 'مسیریابی روی لبه‌های فرومون',
      'learn.loop2.t': 'ویرایش موفق',
      'learn.loop2.s': 'Agent فایل‌های واقعی را ویرایش می‌کند',
      'learn.loop3.t': 'record_feedback',
      'learn.loop3.s': 'تحریک سیناپس — STDP یاد می‌گیرد',
      'learn.loop4.t': 'بستهٔ بعدی',
      'learn.loop4.s': 'مسیر قوی‌تر، خطای کمتر',
      'learn.m1': 'قدرت سیناپسی',
      'learn.m2': 'بازیابی',
      'learn.m3': 'Fallback با grep',
      'learn.c1.t': 'تجربه از ویرایش‌ها',
      'learn.c1.p': 'فقط مسیرهایی که agent <em>واقعاً ویرایش کرده</em> امتیاز می‌گیرند. resolve یکتا، صداقت گراف را حفظ می‌کند.',
      'learn.c2.t': 'لبهٔ فرومون',
      'learn.c2.p': 'فایل‌های ویرایش‌شدهٔ مشترک، لولهٔ قوی‌تری دارند — مثل کپک مخاطی که مسیر غذا را به خاطر می‌سپارد.',
      'learn.c3.t': 'حافظهٔ Mycelial',
      'learn.c3.p': 'Hyphal prefetch، پرش بعدی را در RAM گرم می‌کند. مش فقط مسیر یاد نمی‌گیرد — آنچه expand می‌کنید را از قبل آماده می‌کند.',
      'learn.quote': '<strong>با هم فعال، با هم سیم‌کشی.</strong> بدون <code>record_feedback</code>، بستهٔ بعدی از صفر شروع می‌کند.',

      'stats.label': 'اندازه‌گیری',
      'stats.title': 'آنچه واقعاً اندازه گرفتیم',
      'stats.lead': 'صرفه‌جویی به ازای هر task، بعد از fold. برای تکرار: <code>neuromesh eval</code>',
      'stats.sub': 'انتشار 2026-08-28 v0.7.17 · 650,859 توکن workspace',
      'stats.s1': '٪ نسبت به workspace (handle_tool_call)',
      'stats.s2': '٪ نسبت به workspace (physarum)',
      'stats.s3': 'فایل index‌شده',
      'stats.s4': 'میلی‌ثانیه فعال‌سازی بسته',

      'install.label': 'شروع',
      'install.title': 'نصب',
      'install.lead': 'یک دستور. بعد <code>neuromesh doctor</code> و <code>neuromesh connect</code>.',
      'install.tabMac': 'macOS / Linux',
      'install.tabWin': 'Windows',
      'install.tabSource': 'از سورس',
      'install.tabCargo': 'Cargo',
      'install.after': 'بعد از نصب',
      'install.terminal': 'ترمینال',
      'install.powershell': 'PowerShell',
      'install.buildSource': 'ساخت از سورس (rustup 1.80+)',
      'install.cargoInstall': 'نصب با Cargo',
      'connect.snippet': 'نمونهٔ پیکربندی MCP',

      'connect.label': 'یکپارچه‌سازی',
      'connect.title': 'اتصال',
      'connect.lead': '<strong>MCP stdio</strong> بومی — Cursor، Claude، Codex، OpenCode، MiMo CLI، Antigravity، VS Code، Kilo Code، Trae، Windsurf، Cline/Roo، Zed و بیشتر. <code>neuromesh connect</code> مسیر absolute باینری را می‌نویسد؛ agent به PATH نیاز ندارد.',
      'connect.note': 'دستور <code>neuromesh connect</code> برای appهای نصب‌شده config می‌نویسد، یا <code>neuromesh connect --print</code> برای snippet.',

      'tools.label': 'ابزار MCP',
      'tools.title': 'ابزارها',
      'tools.lead': 'به agent بگویید: context بگیر → در صورت نیاز fold را باز کن → trace → record_feedback.',
      'tools.footer': 'Rust، TypeScript، Python، Go، Java، Kotlin، PHP، C#، Dart، Swift، Ruby — tree-sitter. overlay فریم‌ورک برای Laravel، Django، Next، Nuxt، Spring، Android و بیش از ۳۰ مورد.',

      'docs.label': 'بیشتر بدانید',
      'docs.title': 'مستندات',
      'docs.d1.t': 'سیستم‌های زنده',
      'docs.d1.s': 'DNA، Physarum، STDP — نگاشت به crateها',
      'docs.d2.t': 'معماری',
      'docs.d2.s': 'Pipeline و تضمین‌ها',
      'docs.d3.t': 'MCP و CLI',
      'docs.d3.s': 'مرجع ابزارها و دستورات',
      'docs.d4.t': 'راهنمای agent',
      'docs.d4.s': 'راه‌اندازی برای هر IDE',
      'docs.d5.t': 'کیفیت',
      'docs.d5.s': 'Gold، eval، اعداد',
      'docs.d6.t': 'مشارکت',
      'docs.d6.s': 'ساخت solver یا زبان جدید',

      'footer.tagline': 'NeuroMesh · کد اضافه را حذف نکن؛ تا کن.',
      'footer.changelog': 'Changelog',

      'ui.copy': 'کپی',
      'ui.copied': 'کپی شد!',
      'ui.failed': 'خطا',
      'ui.lightbox': 'پیش‌نمایش تصویر',
      'ui.close': 'بستن',
      'ui.prev': 'قبلی',
      'ui.next': 'بعدی',
      'ui.lang': 'زبان',
      'ui.langEn': 'English',
      'ui.langFa': 'فارسی',
    },
  };

  const phrases = {
    en: [
      "Don't delete the extra code. Fold it.",
      'Route first, then fold.',
      'Structure stays. Tokens sleep.',
    ],
    fa: [
      'کد اضافه را حذف نکن؛ تا کن.',
      'اول مسیریابی، بعد تا.',
      'ساختار می‌ماند. توکن‌ها می‌خوابند.',
    ],
  };

  const pipelineSteps = {
    en: [
      { icon: '💬', title: 'Prompt', phase: 'read', key: 'pipe.0',
        desc: 'Read the task exactly as written — handle_tool_call intent survives; it is not lowercased into mush.' },
      { icon: '🔍', title: 'Identifiers', phase: 'read', key: 'pipe.1',
        desc: 'Extract symbol names and paths from the prompt. Ambiguous names stay sleepy — never a million fake links.' },
      { icon: '🕸️', title: 'Graph in RAM', phase: 'route', key: 'pipe.2',
        desc: 'Resolve on the neural mesh: files, functions, Imports, Calls. Edges exist only when the target is unique.' },
      { icon: '🌱', title: 'Seed files', phase: 'route', key: 'pipe.3',
        desc: 'The files that own those symbols always go in. Seeds are never truncated to fake a small packet.' },
      { icon: '🦠', title: 'Physarum tubes', phase: 'route', key: 'pipe.4',
        desc: 'With two or more seeds, slime mold grows the cheapest connecting tissue on a neighborhood subgraph — under 20ms.' },
      { icon: '⚡', title: 'Fill + synapses', phase: 'splice', key: 'pipe.5',
        desc: 'Callees and synaptic neighbors fill a real token budget. balanced = 5k extra; max_quality = 16k.' },
      { icon: '🧬', title: 'Exon / intron splice', phase: 'splice', key: 'pipe.6',
        desc: 'Untargeted bodies collapse to one-line fold markers. You still see signatures, imports, and the shape of the file.' },
      { icon: '📦', title: 'Evidence packet', phase: 'splice', key: 'pipe.7',
        desc: 'Ship the compact packet to the agent. Wake a fold with neuromesh_expand_fold — then record_feedback after a good edit.' },
    ],
    fa: [
      { icon: '💬', title: 'Prompt', phase: 'read', key: 'pipe.0',
        desc: 'Task دقیقاً همان‌طور که نوشته شده خوانده می‌شود — intentِ handle_tool_call زنده می‌ماند و خرد نمی‌شود.' },
      { icon: '🔍', title: 'Identifiers', phase: 'read', key: 'pipe.1',
        desc: 'نام symbol و path از prompt استخراج می‌شود. نام‌های مبهم «خواب» می‌مانند — نه لینک جعلی انبوه.' },
      { icon: '🕸️', title: 'Graph in RAM', phase: 'route', key: 'pipe.2',
        desc: 'Resolve روی مش عصبی: فایل، تابع، Imports، Calls. لبه فقط وقتی ساخته می‌شود که target یکتا باشد.' },
      { icon: '🌱', title: 'Seed files', phase: 'route', key: 'pipe.3',
        desc: 'فایل‌های صاحب symbol همیشه در بسته می‌آیند. seed برای کوچک‌نمایی بریده نمی‌شود.' },
      { icon: '🦠', title: 'Physarum tubes', phase: 'route', key: 'pipe.4',
        desc: 'با دو seed یا بیشتر، کپک مخاطی کوتاه‌ترین بافت اتصال را روی subgraph همسایگی می‌سازد — زیر ۲۰ms.' },
      { icon: '⚡', title: 'Fill + synapses', phase: 'splice', key: 'pipe.5',
        desc: 'callee و همسایهٔ سیناپسی بودجهٔ token واقعی را پر می‌کنند. balanced = 5k اضافه؛ max_quality = 16k.' },
      { icon: '🧬', title: 'Exon / intron', phase: 'splice', key: 'pipe.6',
        desc: 'بدنه‌های off-target به marker fold یک‌خطی جمع می‌شوند. امضا، import و شکل فایل حفظ می‌ماند.' },
      { icon: '📦', title: 'Evidence packet', phase: 'splice', key: 'pipe.7',
        desc: 'بستهٔ فشرده به agent. fold را با neuromesh_expand_fold بیدار کن — بعد از ویرایش موفق، record_feedback.' },
    ],
  };

  const pipeNodeKeys = {
    en: [
      { title: 'Prompt', key: 'handle_tool_call survives' },
      { title: 'Identifiers', key: 'symbols from task text' },
      { title: 'Graph in RAM', key: 'Files · Calls · Imports' },
      { title: 'Seed files', key: 'owners of symbols' },
      { title: 'Physarum tubes', key: 'cheapest path < 20ms' },
      { title: 'Fill + synapses', key: 'token budget · pheromones' },
      { title: 'Exon / intron', key: 'bodies → fold markers' },
      { title: 'Evidence packet', key: 'expand_fold if needed' },
    ],
    fa: [
      { title: 'Prompt', key: 'intent زنده می‌ماند' },
      { title: 'Identifiers', key: 'symbol از متن task' },
      { title: 'Graph in RAM', key: 'فایل · Call · Import' },
      { title: 'Seed files', key: 'صاحب symbolها' },
      { title: 'Physarum tubes', key: 'کوتاه‌ترین مسیر < 20ms' },
      { title: 'Fill + synapses', key: 'بودجهٔ token · فرومون' },
      { title: 'Exon / intron', key: 'بدنه → marker fold' },
      { title: 'Evidence packet', key: 'expand_fold در صورت نیاز' },
    ],
  };

  const learningTurns = {
    en: [
      { caption: 'First visit — the mesh explores weaker paths. Agent greps twice.' },
      { caption: 'After record_feedback — co-edited route reinforced. One grep left.' },
      { caption: 'Third turn — direct synapse wins. Zero grep. Gold recall.' },
    ],
    fa: [
      { caption: 'اولین بازدید — مش مسیرهای ضعیف‌تر را امتحان می‌کند. Agent دو بار grep می‌زند.' },
      { caption: 'بعد از record_feedback — مسیر ویرایش مشترک تقویت شد. یک grep باقی مانده.' },
      { caption: 'نوبت سوم — سیناپس مستقیم برنده شد. grep صفر. recall طلایی.' },
    ],
  };

  let lang = localStorage.getItem(STORAGE_KEY) || 'en';
  const listeners = [];

  function t(key) {
    return (dict[lang] && dict[lang][key]) || dict.en[key] || key;
  }

  function applyDynamic() {
    const nodes = getPipeNodeKeys();
    document.querySelectorAll('[data-pipe-title]').forEach(el => {
      const i = +el.dataset.pipeTitle;
      if (nodes[i]) el.textContent = nodes[i].title;
    });
    document.querySelectorAll('[data-pipe-key]').forEach(el => {
      const i = +el.dataset.pipeKey;
      if (nodes[i]) el.textContent = nodes[i].key;
    });
    const cap = document.getElementById('learning-turn-caption');
    if (cap && !cap.dataset.userTurn) {
      cap.textContent = t('learn.turnCap1');
    }
    document.querySelectorAll('[data-i18n-caption]').forEach(el => {
      const c = t(el.dataset.i18nCaption);
      el.dataset.caption = c;
    });
    document.querySelectorAll('.gallery-caption[data-i18n]').forEach(el => {
      el.textContent = t(el.dataset.i18n);
    });
  }

  function syncLangDropdown() {
    const root = document.getElementById('lang-dropdown');
    const trigger = document.getElementById('lang-trigger');
    const label = document.getElementById('lang-trigger-label');
    if (label) label.textContent = lang === 'fa' ? t('ui.langFa') : t('ui.langEn');
    if (trigger) {
      trigger.setAttribute('aria-label', t('ui.lang'));
    }
    document.querySelectorAll('.lang-option').forEach(opt => {
      const selected = opt.dataset.lang === lang;
      opt.classList.toggle('active', selected);
      opt.setAttribute('aria-selected', selected ? 'true' : 'false');
    });
    if (root && root.classList.contains('open')) {
      closeLangMenu();
    }
  }

  function openLangMenu() {
    const root = document.getElementById('lang-dropdown');
    const trigger = document.getElementById('lang-trigger');
    const menu = document.getElementById('lang-menu');
    if (!root || !trigger || !menu) return;
    root.classList.add('open');
    menu.hidden = false;
    trigger.setAttribute('aria-expanded', 'true');
  }

  function closeLangMenu() {
    const root = document.getElementById('lang-dropdown');
    const trigger = document.getElementById('lang-trigger');
    const menu = document.getElementById('lang-menu');
    if (!root || !trigger || !menu) return;
    root.classList.remove('open');
    menu.hidden = true;
    trigger.setAttribute('aria-expanded', 'false');
  }

  function initLangDropdown() {
    const root = document.getElementById('lang-dropdown');
    const trigger = document.getElementById('lang-trigger');
    const menu = document.getElementById('lang-menu');
    if (!root || !trigger || !menu) return;

    trigger.addEventListener('click', (e) => {
      e.stopPropagation();
      if (root.classList.contains('open')) closeLangMenu();
      else openLangMenu();
    });

    menu.querySelectorAll('.lang-option').forEach(opt => {
      opt.addEventListener('click', () => {
        setLang(opt.dataset.lang);
        closeLangMenu();
      });
    });

    document.addEventListener('click', (e) => {
      if (!root.contains(e.target)) closeLangMenu();
    });

    document.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') closeLangMenu();
    });
  }

  function applyStatic() {
    document.querySelectorAll('[data-i18n]').forEach(el => {
      el.textContent = t(el.dataset.i18n);
    });
    document.querySelectorAll('[data-i18n-html]').forEach(el => {
      el.innerHTML = t(el.dataset.i18nHtml);
    });
    document.querySelectorAll('[data-i18n-caption]').forEach(el => {
      const cap = t(el.dataset.i18nCaption);
      el.dataset.caption = cap;
      if (el.dataset.lightbox) el.setAttribute('aria-label', cap);
    });
    document.querySelectorAll('[data-i18n-alt]').forEach(el => {
      el.alt = t(el.dataset.i18nAlt);
    });
    document.querySelectorAll('[data-i18n-aria]').forEach(el => {
      el.setAttribute('aria-label', t(el.dataset.i18nAria));
    });
    document.title = t('meta.title');
    const meta = document.querySelector('meta[name="description"]');
    if (meta) meta.content = t('meta.description');
    document.documentElement.lang = lang === 'fa' ? 'fa' : 'en';
    document.documentElement.dir = lang === 'fa' ? 'rtl' : 'ltr';
    document.body.classList.toggle('lang-fa', lang === 'fa');
    document.body.classList.toggle('lang-en', lang === 'en');

    syncLangDropdown();

    document.querySelectorAll('.copy-btn').forEach(btn => {
      if (!btn.classList.contains('copied')) btn.textContent = t('ui.copy');
    });

    const lb = document.getElementById('lightbox');
    if (lb) lb.setAttribute('aria-label', t('ui.lightbox'));
    const close = document.getElementById('lightbox-close');
    if (close) close.setAttribute('aria-label', t('ui.close'));
    const prev = document.getElementById('lightbox-prev');
    if (prev) prev.setAttribute('aria-label', t('ui.prev'));
    const next = document.getElementById('lightbox-next');
    if (next) next.setAttribute('aria-label', t('ui.next'));
    applyDynamic();
  }

  function setLang(next) {
    if (!dict[next]) return;
    lang = next;
    localStorage.setItem(STORAGE_KEY, lang);
    applyStatic();
    listeners.forEach(fn => fn(lang));
  }

  function onChange(fn) {
    listeners.push(fn);
  }

  function getPhrases() { return phrases[lang] || phrases.en; }
  function getPipelineSteps() { return pipelineSteps[lang] || pipelineSteps.en; }
  function getPipeNodeKeys() { return pipeNodeKeys[lang] || pipeNodeKeys.en; }
  function getLearningTurns() { return learningTurns[lang] || learningTurns.en; }

  initLangDropdown();
  setLang(lang);

  return { t, setLang, onChange, applyDynamic, get lang() { return lang; }, getPhrases, getPipelineSteps, getPipeNodeKeys, getLearningTurns };
})();
