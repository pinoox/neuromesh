/* NeuroMesh landing — EN (default) + FA (RTL) */
window.NMI18n = (function () {
  const STORAGE_KEY = 'neuromesh-lang';

  const dict = {
    en: {
      'meta.title': "NeuroMesh — Ship less context. Ship the right code.",
      'meta.description': 'MCP context engine for Cursor, Claude, and VS Code: find the files that matter, fold the rest, and stop paying for thousand-line dumps.',

      'nav.pain': 'Why',
      'nav.fold': 'Fold',
      'nav.galaxy': 'Explore',
      'nav.learning': 'Learns',
      'nav.engines': 'Setup',
      'nav.install': 'Install',
      'nav.connect': 'Connect',
      'nav.tools': 'Tools',
      'nav.github': 'GitHub',
      'nav.installBtn': 'Get started',

      'hero.slimeTag': 'Built for AI-assisted coding',
      'hero.badgeRelease': 'v0.8.6 — bundled MiniLM · zero-config embed',
      'hero.desc': 'Ask in plain language — Persian, Arabic, English, anything. NeuroMesh <strong>embeds your prompt with bundled MiniLM</strong>, finds the right symbols on a live code graph, and sends a <strong>folded evidence packet</strong> instead of three thousand-line file dumps. No keyword tables. No HuggingFace download at install. Works with Cursor, Claude, Codex, VS Code, and every MCP client.',
      'hero.ctaInstall': 'Install in 30 seconds',
      'hero.ctaDocs': 'Read the docs',
      'hero.ctaStar': 'Star on GitHub',
      'hero.clients': 'Works with ·',
      'hero.clientsList': 'Cursor · VS Code · Claude · Codex · OpenCode · Windsurf · Zed · Kilo · Trae · Antigravity · MiMo CLI',
      'hero.foldHeader': 'What the model actually sees — folded skeleton',

      'panel.graph': 'Finds the path',
      'panel.code': 'Folds the noise',
      'panel.engine': 'Builds your packet',

      'pain.label': 'Sound familiar?',
      'pain.title': 'Your editor is oversharing',
      'pain.lead': 'You ask one focused question. The agent pulls in two or three <strong>massive files</strong>, burns tokens, and still misses the function you cared about.',
      'pain.c1.title': 'You pay for noise',
      'pain.c1.desc': 'Private helpers, legacy parsers, and unrelated utilities — billed on every turn.',
      'pain.c2.title': 'You wait for the wrong stuff',
      'pain.c2.desc': 'The chat fills up while the model still has not seen the code path you asked about.',
      'pain.c3.title': 'The model hallucinates',
      'pain.c3.desc': 'Too much context. Wrong conclusions. Bugs that were never in your codebase.',
      'pain.th.approach': 'What people try',
      'pain.th.wrong': 'Why it fails',
      'pain.r1.a': 'Vector RAG',
      'pain.r1.b': 'Chunks break functions apart. Structure disappears.',
      'pain.r2.a': '"Attach the whole file"',
      'pain.r2.b': 'The model sees everything. Understands nothing.',
      'pain.r3.a': 'Static code graphs',
      'pain.r3.b': 'Nice map — then they still paste <strong>full files</strong> into the prompt.',
      'pain.r4.a': 'NeuroMesh',
      'pain.r4.b': '<strong>Route, then fold.</strong> Only the relevant skeleton reaches the model. Expand a fold when you need the body.',

      'fold.label': 'How it works',
      'fold.title': 'Keep structure. Cut tokens.',
      'fold.lead': 'NeuroMesh never deletes your code. It <strong>folds</strong> what the task does not need into a single reversible line — like a table of contents that still knows where every function lives.',
      'fold.c1.title': 'What you need — open',
      'fold.c1.desc': 'The functions and files that match your task stay fully visible with real bodies.',
      'fold.c2.title': 'What you do not — folded',
      'fold.c2.desc': 'Everything else becomes a one-line marker with a fold ID. Signatures and imports stay in view.',
      'fold.c3.title': 'Need the body? One tool call',
      'fold.c3.desc': 'Run <code style="color:var(--purple-fg)">neuromesh_expand_fold</code> — instant restore from memory, no grep hunt.',
      'fold.quote': '<strong>You keep the map. The model stops reading the encyclopedia.</strong>',
      'fold.bio.label': 'Under the hood',
      'fold.bio.title': 'Inspired by how nature packs information',

      'nature.n1.t': 'Smart folding',
      'nature.n1.e': 'Genetic skeletonizer',
      'nature.n1.p': 'Hide boilerplate. Keep signatures, imports, and file shape.',
      'nature.n2.t': 'Shortest path',
      'nature.n2.e': 'Graph routing',
      'nature.n2.p': 'Connect only the files your task needs — not the whole repo neighborhood.',
      'nature.n3.t': 'Remembers what worked',
      'nature.n3.e': 'Learning from your edits',
      'nature.n3.p': 'Files you actually changed get priority on the next similar task.',
      'nature.n4.t': 'Safety modes',
      'nature.n4.e': 'QualityGate',
      'nature.n4.p': 'Balanced by default. Auth and payment tasks automatically get more context.',
      'nature.n5.t': 'Faster follow-ups',
      'nature.n5.e': 'Prefetch',
      'nature.n5.p': 'The next likely file is warmed before your second tool call.',
      'nature.n6.t': 'Live code graph',
      'nature.n6.e': 'Indexed in RAM',
      'nature.n6.p': 'Functions, imports, and calls — so routing is structural, not guesswork.',

      'galaxy.label': 'See your repo',
      'galaxy.title': '3D Neural Galaxy',
      'galaxy.lead': 'Run <code style="color:var(--accent-fg)">neuromesh monitor</code> and explore your project as a live graph — subsystems, files, symbols — before you ask the agent anything.',
      'galaxy.c1': 'Bird\'s-eye view of crates and modules',
      'galaxy.c2': 'File graph with call/import links',
      'galaxy.c3': 'Zoom into one module and its symbols',
      'galaxy.url': 'Default URL:',
      'galaxy.port': 'Port:',

      'how.label': 'Your workflow',
      'how.title': 'One question. One tight packet.',
      'how.lead': 'Ask in plain language (English, Persian, Arabic, …). NeuroMesh finds symbols, pulls neighbors, folds the rest, and tells you when to grep for more.',
      'how.phase1.title': 'Understand',
      'how.phase1.sub': 'Your prompt as written',
      'how.phase2.title': 'Route',
      'how.phase2.sub': 'Graph + smart fill',
      'how.phase3.title': 'Deliver',
      'how.phase3.sub': 'Fold + packet',
      'how.detail.default': 'Your task text is read as-is — identifiers and intent stay intact.',
      'how.stepsOf': 'of 8 steps',
      'how.pauseTour': '⏸ Pause tour',
      'how.playTour': '▶ Play tour',
      'how.mode1': 'Tiny, obvious edits',
      'how.mode2': 'Everyday development',
      'how.mode3': 'Refactors, auth, critical paths',
      'how.extraTokens': 'extra tokens',
      'how.tagDefault': 'Default',

      'learn.label': 'Gets smarter',
      'learn.title': 'The more you ship, the better it routes',
      'learn.lead': 'After a successful edit, call <code>neuromesh_record_feedback</code>. NeuroMesh strengthens the paths you actually used — so the next similar task needs fewer greps and fewer tokens.',
      'learn.turn': 'Turn',
      'learn.turnOf': '/ 3',
      'learn.turnCap1': 'First time — the agent explores and may grep twice.',
      'learn.loopTitle': 'The loop that makes it stick',
      'learn.loop1.t': 'get_context_packet',
      'learn.loop1.s': 'Agent gets a focused packet',
      'learn.loop2.t': 'You edit & succeed',
      'learn.loop2.s': 'Real files, real fix',
      'learn.loop3.t': 'record_feedback',
      'learn.loop3.s': 'Tell NeuroMesh what worked',
      'learn.loop4.t': 'Next task',
      'learn.loop4.s': 'Shorter path, fewer misses',
      'learn.m1': 'Route strength',
      'learn.m2': 'Hit rate',
      'learn.m3': 'Extra greps',
      'learn.c1.t': 'Learns from real edits',
      'learn.c1.p': 'Only files you <em>actually changed</em> influence the next packet — no fake shortcuts.',
      'learn.c2.t': 'Shared paths stick',
      'learn.c2.p': 'Files you often edit together stay linked — the graph prefers routes that worked before.',
      'learn.c3.t': 'Ready for the next hop',
      'learn.c3.p': 'Likely follow-up files are pre-warmed so expand and trace feel instant.',
      'learn.quote': '<strong>Skip feedback and every session starts from zero.</strong> One line after a good fix compounds over time.',

      'stats.label': 'Real numbers',
      'stats.title': 'Measured on a large Rust workspace',
      'stats.lead': 'Token savings are <em>per task</em>, after folding — not a marketing average. Run <code>neuromesh eval</code> on your repo.',
      'stats.sub': 'v0.8.6 · MiniLM embeddings · up to ~97% fewer tokens vs full workspace dump',
      'stats.s1': '% token savings (typical MCP handler task)',
      'stats.s2': '% token savings (graph routing task)',
      'stats.s3': 'Files indexed in demo repo',
      'stats.s4': 'ms to build a packet (warm)',

      'engines.label': 'Defaults',
      'engines.title': 'Embed-first. Everything else is opt-in.',
      'engines.lead': 'v0.8.6 ships <strong>bundled MiniLM</strong> weights in every release. Install, index, pass your prompt — done. Optional: CBM proxy for <code>get_context_packet</code>; lexical/hybrid seed engines for keyword assist.',
      'engines.graphTitle': 'Graph backend',
      'engines.graphDesc': '<code>native</code> (default) — AST index in RAM. <code>auto</code> / <code>proxy_cbm</code> only if you already use codebase-memory-mcp.',
      'engines.seedTitle': 'Prompt → symbols',
      'engines.seedDesc': '<strong>Default: bundled MiniLM embed</strong> — no client keywords. Custom: <code>keywords_expanded</code> or <code>hybrid</code> via <code>nm.config.json</code>.',
      'engines.docsLink': 'Full setup guide →',

      'install.label': 'Get started',
      'install.title': 'Install in one command',
      'install.lead': 'Pre-built binary for macOS, Linux, and Windows — <strong>no Rust required</strong>. Release includes bundled MiniLM weights (~50–80 MB) — no separate download. Then <code>neuromesh doctor</code> and <code>neuromesh connect</code>.',
      'install.tabMac': 'macOS / Linux (recommended)',
      'install.tabWin': 'Windows (recommended)',
      'install.tabSource': 'From source',
      'install.tabCargo': 'Cargo (advanced)',
      'install.after': 'After install',
      'install.terminal': 'Terminal',
      'install.powershell': 'PowerShell',
      'install.buildSource': 'Build from source (rustup 1.80+)',
      'install.cargoInstall': 'Cargo install',
      'connect.snippet': 'MCP config snippet',

      'connect.label': 'Plug in your editor',
      'connect.title': 'Connect in one command',
      'connect.lead': 'NeuroMesh speaks <strong>MCP over stdio</strong> — the standard protocol Cursor, Claude, VS Code, Windsurf, Zed, and dozens of agents already support. No cloud. No API key for indexing. Your code stays on your machine.',
      'connect.note': 'Run <code>neuromesh connect</code> and pick your apps — or <code>neuromesh connect --print</code> to copy a snippet manually.',

      'tools.label': 'Agent tools',
      'tools.title': 'What your agent can call',
      'tools.lead': 'Teach the agent this loop: <strong>get_context_packet</strong> → expand a fold if needed → trace callers → record_feedback after a good edit.',
      'tools.footer': '30+ languages and frameworks out of the box — Rust, TypeScript, Python, Go, PHP, Laravel, Django, Next, Vue, Spring, Android, Express, and more.',

      'tools.t0': 'Start here — compact skeletons, fold IDs, coverage hints',
      'tools.t1': 'Restore one folded function body — no disk grep',
      'tools.t2': 'Follow call and import chains',
      'tools.t3': 'Find symbols when coverage is partial',
      'tools.t4': 'See what a file imports and calls',
      'tools.t5': 'Blast radius before you refactor',
      'tools.t6': 'Languages, packages, entry points at a glance',
      'tools.t7': 'One line after a good edit — routes improve next time',
      'tools.t8': 'Project facts from manifests and docs',
      'tools.t9': 'Debug why a file was selected or dropped',
      'tools.t10': 'Fold a single file on demand',
      'tools.t11': 'Graph size and index health',

      'docs.label': 'Go deeper',
      'docs.title': 'Documentation',
      'docs.d1.t': 'Why biology?',
      'docs.d1.s': 'The ideas behind folding and routing',
      'docs.d2.t': 'Architecture',
      'docs.d2.s': 'How packets are built — for the curious',
      'docs.d3.t': 'MCP & CLI',
      'docs.d3.s': 'Every tool and command',
      'docs.d4.t': 'Agent setup',
      'docs.d4.s': 'Rules and prompts per IDE',
      'docs.d5.t': 'Benchmarks',
      'docs.d5.s': 'Recall, latency, release gates',
      'docs.d6.t': 'Contributing',
      'docs.d6.s': 'Add a language or improve routing',

      'footer.tagline': 'NeuroMesh · Ship less context. Ship the right code.',
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
      'meta.title': 'NeuroMesh — کمتر context بفرست. کد درست را بفرست.',
      'meta.description': 'موتور context برای Cursor، Claude و VS Code: فایل‌های مرتبط را پیدا کن، بقیه را تا کن، دیگر هزار خط بی‌ربط نفرست.',

      'nav.pain': 'چرا؟',
      'nav.fold': 'تا کردن',
      'nav.galaxy': 'کاوش',
      'nav.learning': 'یاد می‌گیرد',
      'nav.engines': 'تنظیمات',
      'nav.install': 'نصب',
      'nav.connect': 'اتصال',
      'nav.tools': 'ابزارها',
      'nav.github': 'GitHub',
      'nav.installBtn': 'شروع کن',

      'hero.slimeTag': 'برای کدنویسی با AI ساخته شده',
      'hero.badgeRelease': 'نسخه 0.8.6 — MiniLM باندل · embed بدون تنظیم',
      'hero.desc': 'به هر زبانی بپرسید. NeuroMesh با <strong>MiniLM باندل‌شده</strong> prompt را embed می‌کند، symbol درست را روی گراف زنده پیدا می‌کند و <strong>بستهٔ تا‌شده</strong> می‌فرستد — نه dump هزار خطی. بدون جدول keyword. بدون دانلود جدا. Cursor، Claude، VS Code و هر clientِ MCP.',
      'hero.ctaInstall': 'نصب در ۳۰ ثانیه',
      'hero.ctaDocs': 'مستندات',
      'hero.ctaStar': 'ستاره در GitHub',
      'hero.clients': 'سازگار با ·',
      'hero.clientsList': 'Cursor · VS Code · Claude · Codex · OpenCode · Windsurf · Zed · Kilo · Trae · Antigravity · MiMo CLI',
      'hero.foldHeader': 'چیزی که مدل واقعاً می‌بیند — اسکلت تا شده',

      'panel.graph': 'مسیر را پیدا می‌کند',
      'panel.code': 'نویز را تا می‌کند',
      'panel.engine': 'بسته را می‌سازد',

      'pain.label': 'آشناست؟',
      'pain.title': 'ویرایشگر بیش از حد می‌فرستد',
      'pain.lead': 'یک سؤال مشخص می‌پرسید. Agent دو سه <strong>فایل عظیم</strong> می‌کشد داخل چت، توکن می‌سوزاند، و هنوز تابعی که دنبالش بودید را درست نمی‌بیند.',
      'pain.c1.title': 'برای نویز پول می‌دهید',
      'pain.c1.desc': 'Helperهای خصوصی، parser قدیمی، utility بی‌ربط — در هر نوبت حساب می‌شود.',
      'pain.c2.title': 'برای چیز اشتباه منتظر می‌مانید',
      'pain.c2.desc': 'چت پر می‌شود اما مدل هنوز مسیر کدی که پرسیدید را ندیده.',
      'pain.c3.title': 'مدل hallucinate می‌کند',
      'pain.c3.desc': 'Context زیاد. نتیجه غلط. باگ‌هایی که اصلاً در پروژه نبودند.',
      'pain.th.approach': 'چه کارهایی می‌کنند',
      'pain.th.wrong': 'چرا جواب نمی‌دهد',
      'pain.r1.a': 'Vector RAG',
      'pain.r1.b': 'Chunk تابع را می‌شکند. ساختار از بین می‌رود.',
      'pain.r2.a': '«کل فایل را attach کن»',
      'pain.r2.b': 'مدل همه‌چیز را می‌بیند. هیچ‌چیز را درست نمی‌فهمد.',
      'pain.r3.a': 'گراف کد ایستا',
      'pain.r3.b': 'نقشه خوب است — بعد باز هم <strong>فایل کامل</strong> می‌چسباند.',
      'pain.r4.a': 'NeuroMesh',
      'pain.r4.b': '<strong>اول مسیر، بعد تا.</strong> فقط اسکلت مرتبط به مدل می‌رسد. بدنه را با expand باز کنید.',

      'fold.label': 'چطور کار می‌کند',
      'fold.title': 'ساختار بماند. توکن کم شود.',
      'fold.lead': 'NeuroMesh کد را حذف نمی‌کند. آنچه task لازم ندارد را در یک خط <strong>تا می‌کند</strong> — مثل فهرست مطالب که هنوز می‌داند هر تابع کجاست.',
      'fold.c1.title': 'آنچه لازم دارید — باز',
      'fold.c1.desc': 'تابع و فایل‌های مرتبط با task با بدنهٔ واقعی می‌مانند.',
      'fold.c2.title': 'آنچه لازم ندارید — تا شده',
      'fold.c2.desc': 'بقیه marker یک‌خطی با fold ID می‌شوند. امضا و import در جای خود.',
      'fold.c3.title': 'بدنه لازم شد؟ یک tool',
      'fold.c3.desc': '<code style="color:var(--purple-fg)">neuromesh_expand_fold</code> — بازگشت فوری از حافظه، بدون grep.',
      'fold.quote': '<strong>نقشه را نگه دارید. مدل دیگر encyclopedia نمی‌خواند.</strong>',
      'fold.bio.label': 'زیر پوست',
      'fold.bio.title': 'الهام از بسته‌بندی اطلاعات در طبیعت',

      'nature.n1.t': 'تا کردن هوشمند',
      'nature.n1.e': 'اسکلت‌ساز',
      'nature.n1.p': 'Boilerplate پنهان. امضا، import و شکل فایل حفظ.',
      'nature.n2.t': 'کوتاه‌ترین مسیر',
      'nature.n2.e': 'مسیریابی گراف',
      'nature.n2.p': 'فقط فایل‌هایی که task می‌خواهد — نه کل همسایگی repo.',
      'nature.n3.t': 'یاد می‌گیرد چه جواب داد',
      'nature.n3.e': 'یادگیری از ویرایش شما',
      'nature.n3.p': 'فایل‌هایی که واقعاً تغییر دادید، task بعدی اولویت می‌گیرند.',
      'nature.n4.t': 'حالت‌های ایمن',
      'nature.n4.e': 'QualityGate',
      'nature.n4.p': 'پیش‌فرض balanced. Auth و payment خودکار context بیشتر می‌گیرند.',
      'nature.n5.t': 'ادامه سریع‌تر',
      'nature.n5.e': 'Prefetch',
      'nature.n5.p': 'فایل محتمل بعدی قبل از tool دوم گرم می‌شود.',
      'nature.n6.t': 'گراف زنده',
      'nature.n6.e': 'Index در RAM',
      'nature.n6.p': 'تابع، import و call — مسیریابی ساختاری، نه حدس.',

      'galaxy.label': 'repo را ببین',
      'galaxy.title': 'کهکشان عصبی ۳D',
      'galaxy.lead': '<code style="color:var(--accent-fg)">neuromesh monitor</code> را اجرا کنید و پروژه را به‌صورت گراف زنده ببینید — قبل از اینکه از agent چیزی بپرسید.',
      'galaxy.c1': 'نمای کلی crate و ماژول',
      'galaxy.c2': 'گراف فایل با call و import',
      'galaxy.c3': 'زوم روی symbolهای یک ماژول',
      'galaxy.url': 'URL پیش‌فرض:',
      'galaxy.port': 'پورت:',

      'how.label': 'جریان کار شما',
      'how.title': 'یک سؤال. یک بستهٔ دقیق.',
      'how.lead': 'به زبان خودتان بپرسید (فارسی، انگلیسی، عربی، …). NeuroMesh symbol پیدا می‌کند، همسایه می‌آورد، بقیه را تا می‌کند، و می‌گوید کی grep بزنید.',
      'how.phase1.title': 'درک',
      'how.phase1.sub': 'prompt همان‌طور که نوشتید',
      'how.phase2.title': 'مسیریابی',
      'how.phase2.sub': 'گراف + fill هوشمند',
      'how.phase3.title': 'تحویل',
      'how.phase3.sub': 'تا + بسته',
      'how.detail.default': 'متن task دست‌نخورده می‌ماند — identifier و intent خرد نمی‌شود.',
      'how.stepsOf': 'از ۸ مرحله',
      'how.pauseTour': '⏸ توقف تور',
      'how.playTour': '▶ پخش تور',
      'how.mode1': 'ویرایش‌های کوچک و واضح',
      'how.mode2': 'توسعهٔ روزمره',
      'how.mode3': 'Refactor، auth، مسیرهای بحرانی',
      'how.extraTokens': 'توکن اضافه',
      'how.tagDefault': 'پیش‌فرض',

      'learn.label': 'یاد می‌گیرد',
      'learn.title': 'هر fix، مسیر بعدی را بهتر می‌کند',
      'learn.lead': 'بعد از ویرایش موفق، <code>neuromesh_record_feedback</code> را بزنید. NeuroMesh مسیرهایی که واقعاً رفتید را تقویت می‌کند — task مشابه بعدی grep و توکن کمتری می‌خواهد.',
      'learn.turn': 'نوبت',
      'learn.turnOf': '/ ۳',
      'learn.turnCap1': 'بار اول — agent کاوش می‌کند، شاید دو بار grep.',
      'learn.loopTitle': 'حلقه‌ای که ماندگار می‌شود',
      'learn.loop1.t': 'get_context_packet',
      'learn.loop1.s': 'بستهٔ متمرکز',
      'learn.loop2.t': 'ویرایش موفق',
      'learn.loop2.s': 'فایل واقعی، fix واقعی',
      'learn.loop3.t': 'record_feedback',
      'learn.loop3.s': 'بگویید چه جواب داد',
      'learn.loop4.t': 'task بعدی',
      'learn.loop4.s': 'مسیر کوتاه‌تر',
      'learn.m1': 'قدرت مسیر',
      'learn.m2': 'نرخ hit',
      'learn.m3': 'grep اضافه',
      'learn.c1.t': 'از ویرایش واقعی',
      'learn.c1.p': 'فقط فایل‌هایی که <em>واقعاً عوض کردید</em> روی بستهٔ بعد اثر می‌گذارند.',
      'learn.c2.t': 'مسیرهای مشترک',
      'learn.c2.p': 'فایل‌هایی که با هم edit می‌کنید، لینک قوی‌تری می‌گیرند.',
      'learn.c3.t': 'آماده برای قدم بعد',
      'learn.c3.p': 'فایل‌های محتمل بعدی از قبل گرم می‌شوند.',
      'learn.quote': '<strong>بدون feedback هر session از صفر است.</strong> یک خط بعد از fix خوب، در طول زمان جمع می‌شود.',

      'stats.label': 'اعداد واقعی',
      'stats.title': 'روی یک workspace بزرگ Rust',
      'stats.lead': 'صرفه‌جویی <em>به ازای هر task</em> است، بعد از fold — نه میانگین تبلیغاتی. روی repo خودتان <code>neuromesh eval</code> بزنید.',
      'stats.sub': 'نسخه 0.8.6 · MiniLM · تا ~۹۷٪ توکن کمتر نسبت به dump کل workspace',
      'stats.s1': '٪ صرفه‌جویی توکن (task معمول MCP)',
      'stats.s2': '٪ صرفه‌جویی (مسیریابی گراف)',
      'stats.s3': 'فایل index‌شده در demo',
      'stats.s4': 'ms ساخت بسته (warm)',

      'engines.label': 'پیش‌فرض',
      'engines.title': 'اول embed. بقیه اختیاری.',
      'engines.lead': 'v0.8.6 وزن <strong>MiniLM</strong> را داخل release می‌آورد. نصب، index، prompt — تمام. اختیاری: CBM proxy؛ موتور lexical/hybrid.',
      'engines.graphTitle': 'گراف backend',
      'engines.graphDesc': '<code>native</code> (پیش‌فرض). <code>auto</code> / <code>proxy_cbm</code> فقط اگر CBM دارید.',
      'engines.seedTitle': 'prompt → symbol',
      'engines.seedDesc': '<strong>پیش‌فرض: embed با MiniLM باندل</strong> — بدون keyword. سفارشی: <code>keywords_expanded</code> یا <code>hybrid</code>.',
      'engines.docsLink': 'راهنمای کامل تنظیمات →',

      'install.label': 'شروع کن',
      'install.title': 'نصب با یک دستور',
      'install.lead': 'باینری آماده برای macOS، Linux و Windows — <strong>بدون Rust</strong>. وزن MiniLM داخل release باندل شده (~50–80 MB) — دانلود جدا لازم نیست.',
      'install.tabMac': 'macOS / Linux (پیشنهادی)',
      'install.tabWin': 'Windows (پیشنهادی)',
      'install.tabSource': 'از سورس',
      'install.tabCargo': 'Cargo (پیشرفته)',
      'install.after': 'بعد از نصب',
      'install.terminal': 'ترمینال',
      'install.powershell': 'PowerShell',
      'install.buildSource': 'ساخت از سورس (rustup 1.80+)',
      'install.cargoInstall': 'نصب با Cargo',
      'connect.snippet': 'نمونهٔ پیکربندی MCP',

      'connect.label': 'editor را وصل کن',
      'connect.title': 'اتصال با یک دستور',
      'connect.lead': 'NeuroMesh از <strong>MCP روی stdio</strong> استفاده می‌کند — همان پروتکلی که Cursor، Claude، VS Code، Windsurf و Zed از آن پشتیبانی می‌کنند. بدون cloud. بدون API key برای index. کد روی ماشین شما می‌ماند.',
      'connect.note': '<code>neuromesh connect</code> را بزنید و app را انتخاب کنید — یا <code>neuromesh connect --print</code> برای کپی دستی snippet.',

      'tools.label': 'ابزار agent',
      'tools.title': 'چه چیزی را agent صدا می‌زند',
      'tools.lead': 'این حلقه را به agent بدهید: <strong>get_context_packet</strong> → در صورت نیاز expand fold → trace → record_feedback بعد از fix خوب.',
      'tools.footer': '۳۰+ زبان و فریم‌ورک — Rust، TypeScript، Python، Go، PHP، Laravel، Django، Next، Vue، Spring، Android، Express و بیشتر.',

      'tools.t0': 'از اینجا شروع کن — اسکلت فشرده، fold ID، راهنمای coverage',
      'tools.t1': 'بدنهٔ یک تابع تا شده را برگردان — بدون grep',
      'tools.t2': 'زنجیره call و import',
      'tools.t3': 'symbol پیدا کن وقتی coverage ناقص است',
      'tools.t4': 'همسایه‌های import و call',
      'tools.t5': 'شعاع اثر قبل از refactor',
      'tools.t6': 'زبان‌ها، package و entry point',
      'tools.t7': 'یک خط بعد از fix — مسیر بعدی بهتر می‌شود',
      'tools.t8': 'حقایق پروژه از manifest و doc',
      'tools.t9': 'بفهم چرا فایلی انتخاب یا حذف شد',
      'tools.t10': 'یک فایل را on-demand تا کن',
      'tools.t11': 'اندازه گراف و سلامت index',

      'docs.label': 'عمیق‌تر',
      'docs.title': 'مستندات',
      'docs.d1.t': 'چرا biology؟',
      'docs.d1.s': 'ایده پشت folding و routing',
      'docs.d2.t': 'معماری',
      'docs.d2.s': 'چطور بسته ساخته می‌شود',
      'docs.d3.t': 'MCP و CLI',
      'docs.d3.s': 'همه tool و command',
      'docs.d4.t': 'راه‌اندازی agent',
      'docs.d4.s': 'rule و prompt برای هر IDE',
      'docs.d5.t': 'benchmark',
      'docs.d5.s': 'recall، latency، release gate',
      'docs.d6.t': 'مشارکت',
      'docs.d6.s': 'زبان یا routing بهتر',

      'footer.tagline': 'NeuroMesh · کمتر context بفرست. کد درست را بفرست.',
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
      'Ship less context. Ship the right code.',
      'Prompt only. MiniLM finds the symbols.',
      'Route first. Fold the rest.',
      'Your repo as a map — not a dump.',
    ],
    fa: [
      'کمتر context بفرست. کد درست را بفرست.',
      'فقط prompt. MiniLM symbol را پیدا می‌کند.',
      'اول مسیر. بعد fold.',
      'repo مثل نقشه — نه dump.',
    ],
  };

  const pipelineSteps = {
    en: [
      { icon: '💬', title: 'Your prompt', phase: 'read', key: 'pipe.0',
        desc: 'Describe the task in plain language. NeuroMesh keeps identifiers, file hints, and intent as you wrote them.' },
      { icon: '🔍', title: 'Embed & match', phase: 'read', key: 'pipe.1',
        desc: 'Bundled MiniLM embeds your prompt and ANN-searches symbol sketches in the sidecar — any language, no keyword tables.' },
      { icon: '🕸️', title: 'Load the graph', phase: 'route', key: 'pipe.2',
        desc: 'Your repo lives in RAM as structure: files, calls, imports — not shredded text chunks.' },
      { icon: '🌱', title: 'Pick seed files', phase: 'route', key: 'pipe.3',
        desc: 'Files that own the symbols you asked about always enter the packet first. Coverage is honest when uncertain.' },
      { icon: '🦠', title: 'Connect the path', phase: 'route', key: 'pipe.4',
        desc: 'Smart routing links related files on the shortest useful path — neighbors only when the task needs them.' },
      { icon: '⚡', title: 'Fill the budget', phase: 'splice', key: 'pipe.5',
        desc: 'Callees and side files fill a real token budget. You get enough context to edit — not the whole repo.' },
      { icon: '🧬', title: 'Fold the rest', phase: 'splice', key: 'pipe.6',
        desc: 'Everything off-target collapses to a one-line fold marker. Signatures, imports, and file shape stay visible.' },
      { icon: '📦', title: 'Ship the packet', phase: 'splice', key: 'pipe.7',
        desc: 'The agent receives a compact skeleton with fold IDs. Expand a body with neuromesh_expand_fold — then record_feedback after a good fix.' },
    ],
    fa: [
      { icon: '💬', title: 'prompt شما', phase: 'read', key: 'pipe.0',
        desc: 'task را به زبان خودتان بنویسید. NeuroMesh شناسه‌ها، hint فایل و intent را همان‌طور نگه می‌دارد.' },
      { icon: '🔍', title: 'embed و match', phase: 'read', key: 'pipe.1',
        desc: 'MiniLM باندل prompt را embed می‌کند و sketch نمادها را در sidecar جستجو می‌کند — هر زبانی، بدون جدول keyword.' },
      { icon: '🕸️', title: 'گراف را بار کن', phase: 'route', key: 'pipe.2',
        desc: 'repo در RAM به‌صورت ساختار index می‌شود: فایل، call، import — نه chunk متنی.' },
      { icon: '🌱', title: 'فایل seed', phase: 'route', key: 'pipe.3',
        desc: 'فایل‌های صاحب symbol همیشه اول در بسته می‌آیند. وقتی مطمئن نیست، صادقانه می‌گوید.' },
      { icon: '🦠', title: 'مسیر را وصل کن', phase: 'route', key: 'pipe.4',
        desc: 'مسیریابی هوشمند فایل‌های مرتبط را روی کوتاه‌ترین مسیر مفید وصل می‌کند.' },
      { icon: '⚡', title: 'بودجه را پر کن', phase: 'splice', key: 'pipe.5',
        desc: 'callee و فایل‌های کناری تا سقف token واقعی اضافه می‌شوند — نه کل repo.' },
      { icon: '🧬', title: 'بقیه را fold کن', phase: 'splice', key: 'pipe.6',
        desc: 'هر چیز off-target به marker یک‌خطی تبدیل می‌شود. امضا و import در view می‌مانند.' },
      { icon: '📦', title: 'بسته را بفرست', phase: 'splice', key: 'pipe.7',
        desc: 'agent اسکلت فشرده با fold ID می‌گیرد. expand کن — بعد از fix خوب record_feedback بزن.' },
    ],
  };

  const pipeNodeKeys = {
    en: [
      { title: 'Your prompt', key: 'Intent preserved' },
      { title: 'Embed & match', key: 'MiniLM ANN' },
      { title: 'Load the graph', key: 'Calls · imports' },
      { title: 'Pick seed files', key: 'Owners first' },
      { title: 'Connect the path', key: 'Smart routing' },
      { title: 'Fill the budget', key: 'Real token cap' },
      { title: 'Fold the rest', key: 'One-line markers' },
      { title: 'Ship the packet', key: 'Ready for agent' },
    ],
    fa: [
      { title: 'prompt شما', key: 'intent حفظ می‌شود' },
      { title: 'embed و match', key: 'MiniLM ANN' },
      { title: 'گراف را بار کن', key: 'Call · import' },
      { title: 'فایل seed', key: 'صاحب symbol اول' },
      { title: 'مسیر را وصل کن', key: 'مسیریابی هوشمند' },
      { title: 'بودجه را پر کن', key: 'سقف token واقعی' },
      { title: 'بقیه را fold کن', key: 'marker یک‌خطی' },
      { title: 'بسته را بفرست', key: 'آماده برای agent' },
    ],
  };

  const learningTurns = {
    en: [
      { caption: 'First time — the agent explores. Maybe greps twice.' },
      { caption: 'After feedback — your route is reinforced. One grep left.' },
      { caption: 'Third task — direct hit. Zero extra greps.' },
    ],
    fa: [
      { caption: 'بار اول — agent کاوش می‌کند. شاید دو بار grep.' },
      { caption: 'بعد از feedback — مسیر شما تقویت شد. یک grep باقی مانده.' },
      { caption: 'task سوم — hit مستقیم. grep اضافه صفر.' },
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
