const fs = require("fs");
const fsp = require("fs/promises");
const path = require("path");

require("module").Module._initPaths();

const PptxGenJS = require("pptxgenjs");
const sharp = require("sharp");

const ROOT = path.resolve(__dirname, "..");
const OUT_DIR = path.join(ROOT, "presentation", "generated");
const PREVIEW_DIR = path.join(OUT_DIR, "previews");
const PPTX_PATH = path.join(OUT_DIR, "bitcoin-blockchain-indexer-defense.pptx");

const W = 13.333;
const H = 7.5;

const C = {
  ink: "FFF8EC",
  ink2: "FFFFFF",
  graphite: "F2E7D6",
  graphite2: "D8C7AE",
  red: "F7931A",
  red2: "FFE0AD",
  coral: "C86400",
  text: "1C2430",
  muted: "3E4A59",
  dim: "6F7B88",
  steel: "6B7280",
  amber: "F7931A",
  green: "2F7D59",
};

const F = {
  title: "Aptos Display",
  body: "Aptos",
  mono: "Cascadia Code",
};

const assets = {
  c4: path.join(ROOT, "report", "c4", "C4_Component_Architecture.png"),
  dfd1: path.join(ROOT, "report", "dfd", "DFD_Level_1.png"),
  dfdIndex: path.join(ROOT, "report", "dfd", "DFD_Level_2_Indexing.png"),
  qr: path.join(ROOT, "diploma", "inc", "github_qr.png"),
};

function hexToRgb(hex) {
  const v = hex.replace("#", "");
  return [parseInt(v.slice(0, 2), 16), parseInt(v.slice(2, 4), 16), parseInt(v.slice(4, 6), 16)];
}

function escapeXml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function wrapText(text, maxChars) {
  const words = String(text).split(/\s+/);
  const lines = [];
  let line = "";
  for (const word of words) {
    if ((line + " " + word).trim().length > maxChars) {
      if (line) lines.push(line);
      line = word;
    } else {
      line = (line + " " + word).trim();
    }
  }
  if (line) lines.push(line);
  return lines;
}

async function imageBox(imgPath, x, y, w, h) {
  const meta = await sharp(imgPath).metadata();
  const ratio = meta.width / meta.height;
  const boxRatio = w / h;
  if (ratio > boxRatio) {
    const ih = w / ratio;
    return { x, y: y + (h - ih) / 2, w, h: ih };
  }
  const iw = h * ratio;
  return { x: x + (w - iw) / 2, y, w: iw, h };
}

async function imageDataUri(imgPath) {
  const buf = await fsp.readFile(imgPath);
  const ext = path.extname(imgPath).replace(".", "").toLowerCase();
  const mime = ext === "jpg" || ext === "jpeg" ? "image/jpeg" : "image/png";
  return `data:${mime};base64,${buf.toString("base64")}`;
}

function addBg(slide, n, label) {
  slide.background = { color: C.ink };
  slide.addShape("rect", { x: 0, y: 0, w: W, h: H, fill: { color: C.ink }, line: { color: C.ink } });
  slide.addShape("parallelogram", {
    x: -1.05,
    y: -0.25,
    w: 5.7,
    h: 8.0,
    fill: { color: n % 3 === 0 ? C.red2 : C.graphite, transparency: 24 },
    line: { color: n % 3 === 0 ? C.red2 : C.graphite, transparency: 100 },
    rotate: 0,
  });
  slide.addShape("parallelogram", {
    x: 9.55,
    y: -0.5,
    w: 4.4,
    h: 8.3,
    fill: { color: n % 2 === 0 ? C.graphite2 : C.red2, transparency: 44 },
    line: { color: C.graphite2, transparency: 100 },
  });
  for (let i = 0; i < 11; i += 1) {
    slide.addShape("line", {
      x: 0.4 + i * 1.2,
      y: 0,
      w: 0,
      h: H,
      line: { color: "B6A68F", transparency: 78, width: 0.5 },
    });
  }
  slide.addShape("line", { x: 0.6, y: 0.42, w: 12.1, h: 0, line: { color: C.red, width: 1.35 } });
  slide.addShape("line", { x: 0.6, y: 7.04, w: 12.1, h: 0, line: { color: C.graphite2, width: 1 } });
  slide.addText(String(n).padStart(2, "0"), {
    x: 11.95,
    y: 6.83,
    w: 0.55,
    h: 0.22,
    fontFace: F.mono,
    fontSize: 9,
    color: C.dim,
    bold: true,
    margin: 0,
  });
  slide.addText(label || "BITCOIN BLOCKCHAIN INDEXER", {
    x: 0.72,
    y: 6.83,
    w: 4.8,
    h: 0.22,
    fontFace: F.mono,
    fontSize: 8.5,
    color: C.dim,
    margin: 0,
    breakLine: false,
  });
}

function addTitle(slide, kicker, title, subtitle) {
  slide.addText(kicker.toUpperCase(), {
    x: 0.72,
    y: 0.68,
    w: 3.8,
    h: 0.25,
    fontFace: F.mono,
    fontSize: 9.5,
    color: C.coral,
    bold: true,
    margin: 0,
  });
  slide.addText(title, {
    x: 0.72,
    y: 1.03,
    w: 8.1,
    h: 0.62,
    fontFace: F.title,
    fontSize: 32,
    color: C.text,
    bold: true,
    fit: "shrink",
    margin: 0,
  });
  if (subtitle) {
    slide.addText(subtitle, {
      x: 0.74,
      y: 1.66,
      w: 7.4,
      h: 0.32,
      fontFace: F.body,
      fontSize: 15,
      color: C.muted,
      margin: 0,
      fit: "shrink",
    });
  }
}

function addChip(slide, text, x, y, w, color = C.graphite2) {
  slide.addShape("roundRect", {
    x,
    y,
    w,
    h: 0.34,
    rectRadius: 0.04,
    fill: { color, transparency: 8 },
    line: { color: color === C.red ? C.coral : C.graphite2, transparency: 40, width: 0.8 },
  });
  slide.addText(text, {
    x: x + 0.12,
    y: y + 0.08,
    w: w - 0.24,
    h: 0.16,
    fontFace: F.mono,
    fontSize: 8.7,
    color: C.text,
    bold: true,
    margin: 0,
    fit: "shrink",
  });
}

function addBigClaim(slide, claim, x, y, w, h, size = 34) {
  slide.addText(claim, {
    x,
    y,
    w,
    h,
    fontFace: F.title,
    fontSize: size,
    bold: true,
    color: C.text,
    margin: 0,
    fit: "shrink",
    breakLine: false,
  });
}

function addShortList(slide, items, x, y, w, gap = 0.58, opts = {}) {
  const fontSize = opts.fontSize || 16.5;
  const textH = opts.textH || 0.32;
  items.forEach((item, i) => {
    const yy = y + i * gap;
    slide.addShape("line", { x, y: yy + 0.16, w: 0.32, h: 0, line: { color: item.color || C.coral, width: 2 } });
    slide.addText(item.text, {
      x: x + 0.43,
      y: yy,
      w,
      h: textH,
      fontFace: F.body,
      fontSize,
      bold: item.bold || false,
      color: item.colorText || C.text,
      margin: 0,
      fit: "shrink",
    });
  });
}

function addNumberedTasks(slide, tasks, x, y, opts = {}) {
  const gap = opts.gap || 0.58;
  const fontSize = opts.fontSize || 15.4;
  const textW = opts.textW || 4.52;
  const textH = opts.textH || 0.3;
  tasks.forEach((task, i) => {
    const yy = y + i * gap;
    slide.addText(String(i + 1).padStart(2, "0"), {
      x,
      y: yy,
      w: 0.42,
      h: 0.22,
      fontFace: F.mono,
      fontSize: 9.5,
      bold: true,
      color: C.coral,
      margin: 0,
      fit: "shrink",
    });
    slide.addShape("line", { x: x + 0.55, y: yy + 0.12, w: 0.34, h: 0, line: { color: C.steel, width: 1.2 } });
    slide.addText(task, {
      x: x + 1.02,
      y: yy - 0.02,
      w: textW,
      h: textH,
      fontFace: F.body,
      fontSize,
      color: C.text,
      margin: 0,
      fit: "shrink",
    });
  });
}

function addMetric(slide, value, label, x, y, accent = C.coral) {
  slide.addText(value, {
    x,
    y,
    w: 1.65,
    h: 0.52,
    fontFace: F.title,
    fontSize: 34,
    bold: true,
    color: accent,
    margin: 0,
    align: "center",
    fit: "shrink",
  });
  slide.addText(label, {
    x,
    y: y + 0.58,
    w: 1.65,
    h: 0.34,
    fontFace: F.mono,
    fontSize: 8.4,
    color: C.muted,
    margin: 0,
    align: "center",
    fit: "shrink",
  });
}

async function addImagePanel(slide, imgPath, x, y, w, h, caption) {
  slide.addShape("roundRect", {
    x,
    y,
    w,
    h,
    rectRadius: 0.03,
    fill: { color: "FFFFFF", transparency: 0 },
    line: { color: C.graphite2, transparency: 10, width: 1 },
  });
  const box = await imageBox(imgPath, x + 0.16, y + 0.16, w - 0.32, h - 0.52);
  slide.addImage({ path: imgPath, ...box });
  if (caption) {
    slide.addText(caption.toUpperCase(), {
      x: x + 0.18,
      y: y + h - 0.27,
      w: w - 0.36,
      h: 0.13,
      fontFace: F.mono,
      fontSize: 6.8,
      color: C.dim,
      margin: 0,
      fit: "shrink",
    });
  }
}

function addNode(slide, title, subtitle, x, y, w, h, opts = {}) {
  const accent = opts.accent || C.steel;
  slide.addShape("roundRect", {
    x,
    y,
    w,
    h,
    rectRadius: 0.03,
    fill: { color: opts.fill || C.ink2, transparency: opts.transparency ?? 2 },
    line: { color: accent, transparency: opts.lineTransparency ?? 18, width: opts.lineWidth || 1.15 },
  });
  slide.addShape("line", { x: x + 0.16, y: y + 0.16, w: 0.42, h: 0, line: { color: accent, width: 2 } });
  slide.addText(title, {
    x: x + 0.18,
    y: y + 0.34,
    w: w - 0.36,
    h: 0.28,
    fontFace: F.title,
    fontSize: opts.titleSize || 16,
    bold: true,
    color: C.text,
    margin: 0,
    fit: "shrink",
  });
  if (subtitle) {
    slide.addText(subtitle, {
      x: x + 0.18,
      y: y + 0.76,
      w: w - 0.36,
      h: h - 0.86,
      fontFace: F.body,
      fontSize: opts.subtitleSize || 11,
      color: C.muted,
      margin: 0,
      fit: "shrink",
    });
  }
}

function addArrow(slide, x1, y1, x2, y2, color = C.steel) {
  slide.addShape("line", {
    x: x1,
    y: y1,
    w: x2 - x1,
    h: y2 - y1,
    line: { color, width: 1.35, endArrowType: "triangle" },
  });
}

function addArchitectureDiagram(slide) {
  addNode(slide, "Bitcoin Core", "источник блоков\nи mempool", 0.92, 2.65, 2.15, 1.22, { accent: C.amber, titleSize: 16, subtitleSize: 12 });
  addNode(slide, "Backend-модуль", "прикладная логика:\nиндексация, jobs и mempool\nвыдача данных через REST API", 4.05, 2.25, 3.75, 2.08, { accent: C.coral, fill: C.graphite, titleSize: 18, subtitleSize: 12.1 });
  addNode(slide, "PostgreSQL", "критическое состояние:\nблоки, транзакции, UTXO,\nбалансы, jobs", 8.75, 2.65, 2.75, 1.45, { accent: C.green, titleSize: 16, subtitleSize: 11.5 });
  addNode(slide, "Внешние клиенты", "прикладные запросы\nи управление jobs", 4.32, 5.02, 2.95, 1.0, { accent: C.steel, titleSize: 15.5, subtitleSize: 11.2 });

  addArrow(slide, 3.08, 3.25, 4.0, 3.12, C.amber);
  addArrow(slide, 7.72, 3.12, 8.7, 3.25, C.green);
  addArrow(slide, 5.88, 4.28, 5.88, 4.98, C.steel);

}

function addIndexingDiagram(slide) {
  const steps = [
    ["1", "Определение диапазона", "tip цепи и прогресс job", C.amber],
    ["2", "Проверка согласованности", "обработка возможного reorg", C.coral],
    ["3", "Нормализация данных", "блоки, транзакции, входы, выходы", C.green],
    ["4", "Фиксация результата", "UTXO, балансы, progress", C.steel],
  ];
  steps.forEach((st, i) => {
    const x = 0.92 + i * 2.9;
    addNode(slide, `${st[0]}. ${st[1]}`, st[2], x, 2.45, 2.25, 1.28, { accent: st[3], titleSize: 15.4, subtitleSize: 11.2 });
    if (i < steps.length - 1) addArrow(slide, x + 2.25, 2.94, x + 2.78, 2.94, C.steel);
  });
  addNode(slide, "Классы данных", "первичные сущности: блоки и транзакции\nсостояние: UTXO и адресные балансы\nуправление: jobs и сведения об узлах", 0.92, 4.65, 4.85, 1.45, { accent: C.green, titleSize: 17, subtitleSize: 11.4 });
  addNode(slide, "Гарантии обработки", "транзакционная запись\nидемпотентность повторной обработки\nразделение confirmed, mempool и orphaned", 6.45, 4.65, 4.85, 1.45, { accent: C.coral, titleSize: 17, subtitleSize: 11.4 });
}

function addBalanceIndexingDiagram(slide) {
  addNode(slide, "1. Транзакция", "входы ссылаются\nна ранее созданные выходы", 0.92, 2.38, 2.3, 1.18, { accent: C.steel, titleSize: 15.5, subtitleSize: 11.3 });
  addNode(slide, "2. UTXO", "новые выходы: unspent\nпотраченные: spent", 3.55, 2.38, 2.3, 1.18, { accent: C.amber, titleSize: 15.5, subtitleSize: 11.3 });
  addNode(slide, "3. Баланс", "delta(address):\n+ выходы, - входы", 6.18, 2.38, 2.3, 1.18, { accent: C.coral, fill: C.graphite, titleSize: 15.5, subtitleSize: 11.3 });
  addNode(slide, "4. История", "snapshot баланса\nна высоте блока", 8.82, 2.38, 2.55, 1.18, { accent: C.green, titleSize: 15.5, subtitleSize: 11.3 });
  addArrow(slide, 3.22, 2.96, 3.5, 2.96, C.steel);
  addArrow(slide, 5.85, 2.96, 6.12, 2.96, C.steel);
  addArrow(slide, 8.48, 2.96, 8.78, 2.96, C.steel);

  sAddRuleText(slide, "Правило расчета", 0.95, 4.28, C.coral);
  addShortList(slide, [
    { text: "выход с адресом увеличивает баланс на value_sats" },
    { text: "вход тратит найденный UTXO и уменьшает баланс владельца" },
    { text: "после блока сохраняется исторический снимок затронутых адресов" },
  ], 0.98, 4.73, 5.95, 0.43, { fontSize: 14.2, textH: 0.28 });

  addNode(slide, "Проверочный сценарий", "block 0: addr1 = 50 BTC\nblock 1: addr1 = 20 BTC, addr2 = 30 BTC\nистория: 3 снимка балансов", 7.35, 4.34, 4.55, 1.55, { accent: C.green, fill: C.ink2, titleSize: 17, subtitleSize: 12 });
}

function addForkRollbackDiagram(slide) {
  addNode(slide, "Контрольное окно", "сравнение hash_db\nи hash_rpc на глубине reorg", 0.92, 2.42, 2.55, 1.2, { accent: C.amber, titleSize: 15.5, subtitleSize: 11.2 });
  addNode(slide, "Точка расхождения", "первая высота,\nгде хэши различаются", 3.84, 2.42, 2.55, 1.2, { accent: C.coral, fill: C.graphite, titleSize: 15.5, subtitleSize: 11.2 });
  addNode(slide, "Откат хвоста", "blocks и transactions\nполучают status=orphaned", 6.76, 2.42, 2.55, 1.2, { accent: C.steel, titleSize: 15.5, subtitleSize: 11.2 });
  addNode(slide, "Пересборка состояния", "UTXO, current balance\nи history строятся заново", 9.66, 2.42, 2.55, 1.2, { accent: C.green, titleSize: 15.5, subtitleSize: 11.2 });
  addArrow(slide, 3.47, 3.0, 3.78, 3.0, C.steel);
  addArrow(slide, 6.39, 3.0, 6.7, 3.0, C.steel);
  addArrow(slide, 9.31, 3.0, 9.6, 3.0, C.steel);

  sAddRuleText(slide, "Гарантии механизма", 0.95, 4.35, C.coral);
  addShortList(slide, [
    { text: "операция выполняется в транзакции PostgreSQL" },
    { text: "orphaned-данные не участвуют в прикладной выдаче" },
    { text: "балансы восстанавливаются replay-проходом по canonical-блокам" },
  ], 0.98, 4.78, 6.2, 0.43, { fontSize: 14.2, textH: 0.28 });

  addNode(slide, "Проверочный сценарий", "mock RPC возвращает новый hash на высоте 1\nстарый блок и транзакция становятся orphaned\nбаланс addr1 восстанавливается до 50 BTC", 7.55, 4.25, 4.4, 1.62, { accent: C.amber, fill: C.ink2, titleSize: 16.5, subtitleSize: 11.8 });
}

function sAddRuleText(slide, text, x, y, color = C.coral) {
  slide.addText(text.toUpperCase(), {
    x,
    y,
    w: 3.2,
    h: 0.22,
    fontFace: F.mono,
    fontSize: 9.5,
    bold: true,
    color,
    margin: 0,
    fit: "shrink",
  });
}

function addNotes(slide, notes) {
  slide.addNotes(notes.replace(/\n+/g, "\n"));
}

const notes = [
  "0:00-0:20. Представляю тему: это система, которая превращает низкоуровневые данные Bitcoin в управляемый слой хранения и API. На защите я покажу проблему, архитектуру, механизм индексации, тестирование бизнес-функций, замеры производительности и практический результат.",
  "0:20-0:55. Актуальность состоит в том, что Bitcoin Core является доверенным источником данных, но не формирует прикладный слой структурированных представлений: UTXO адресов, историю балансов, фильтрацию mempool и устойчивую выдачу с учетом reorg. Цель ВКР - спроектировать и реализовать систему индексации блокчейн данных, обеспечивающую согласованное хранение и прикладную выдачу данных сети Bitcoin. Для достижения цели решаются шесть задач: анализ предметной области, требования, архитектура, модель данных, реализация модулей и проверка качества с замерами производительности.",
  "0:55-1:35. На слайде приведено сравнение подходов по преимуществам и недостаткам. Bitcoin Core RPC является источником, electrs и Esplora ориентированы на explorer-сценарии, BlockSci - на исследования, Bitcoin ETL - на выгрузки. Следовательно, существующие решения не обеспечивают полного достижения цели и задач ВКР в едином программном комплексе.",
  "1:35-2:05. Требования разделены на функциональные и нефункциональные. Функциональные описывают, какие действия выполняет система: индексацию, UTXO, балансы, mempool, jobs и REST API. Нефункциональные требования описывают качество работы системы и проверяются по статистике испытаний: хвостовая задержка p95 для API, пропускная способность в RPS, доля успешных ответов и наличие метрик наблюдаемости.",
  "2:05-2:40. Архитектурно выбран stateless модульный монолит. Backend-модуль выполняет прикладную логику системы: запускает индексацию, управляет jobs и mempool, а также выдает проиндексированные данные через REST API. Состояние хранится в PostgreSQL, поэтому backend можно перезапускать без потери прогресса.",
  "2:40-3:15. Основной алгоритм представлен обобщенно: определить диапазон обработки, проверить согласованность цепи, нормализовать блокчейн данные и зафиксировать результат в базе. Модель данных разделяет первичные сущности, состояние, управление и наблюдаемость. Mempool не смешивается с подтвержденной цепью.",
  "3:15-3:55. Балансы строятся через UTXO-модель. Каждый новый выход с адресом увеличивает баланс, а каждый вход, который тратит ранее созданный выход, уменьшает баланс владельца этого выхода. После фиксации блока сохраняется актуальное состояние и исторический снимок по высоте блока.",
  "3:55-4:35. При fork или reorg система сравнивает хэши блоков в контрольном окне. Если найдено расхождение, хвост цепи переводится в orphaned, производные состояния очищаются, а UTXO и балансы пересобираются replay-проходом по оставшимся canonical-блокам. Это защищает прикладную выдачу от данных неканонической ветки.",
  "4:35-5:25. Тестирование сфокусировано на бизнес-функциях. Проверены сценарии управления заданиями, прикладной выдачи данных, индексации блоков, расчета UTXO и балансов, обработки mempool и отката при reorg. Результаты фиксируются через ожидаемые HTTP-статусы, JSON-ответы, состояния таблиц и контрольные значения балансов.",
  "5:25-6:05. Для оценки производительности сравнивались прямые Bitcoin Core RPC-запросы и чтение из заранее подготовленного PostgreSQL-индекса. На слайде показана хвостовая задержка p95: она лучше отражает устойчивость ответа при повторяющихся запросах. Профиль нагрузки: 50 прогревочных и 1000 измеряемых запросов на короткий сценарий. Наибольший выигрыш получен для UTXO и балансов адреса.",
  "6:05-6:35. Результаты работы соответствуют шести задачам ВКР: предметная область проанализирована, требования сформированы, архитектура обоснована, модель данных спроектирована, ключевые модули реализованы, качество проверено бизнес-сценариями. QR ведет на GitHub проекта.",
  "6:35-7:00. Практическая ценность в том, что система снижает сложность интеграции внешних сервисов с Bitcoin-данными. Она дает нормализованные прикладные ответы и может развиваться в сторону explorer, аналитики, мониторинга и промышленного эксплуатационного контура.",
];

async function buildPptx() {
  await fsp.mkdir(PREVIEW_DIR, { recursive: true });

  const pptx = new PptxGenJS();
  pptx.layout = "LAYOUT_WIDE";
  pptx.author = "Ефименко Кирилл Игоревич";
  pptx.company = "МАИ";
  pptx.subject = "Защита ВКР";
  pptx.title = "Система индексации блокчейн данных";
  pptx.lang = "ru-RU";
  pptx.theme = {
    headFontFace: F.title,
    bodyFontFace: F.body,
    lang: "ru-RU",
  };
  pptx.margin = 0;

  // 1
  {
    const s = pptx.addSlide();
    addBg(s, 1, "ВКР / 7 минут");
    s.addShape("rect", { x: 8.8, y: 0, w: 0.12, h: H, fill: { color: C.red }, line: { color: C.red } });
    s.addText("BITCOIN\nBLOCKCHAIN\nINDEXER", {
      x: 0.72,
      y: 0.85,
      w: 5.3,
      h: 1.65,
      fontFace: F.mono,
      fontSize: 24,
      bold: true,
      color: C.muted,
      margin: 0,
      breakLine: false,
      fit: "shrink",
    });
    addBigClaim(s, "Система индексации\nблокчейн данных", 0.72, 2.65, 7.3, 1.55, 36);
    s.addText("ВКР бакалавра / М8О-409Б-22", {
      x: 0.76,
      y: 4.55,
      w: 5.4,
      h: 0.28,
      fontFace: F.body,
      fontSize: 16,
      color: C.muted,
      margin: 0,
    });
    addChip(s, "Ефименко Кирилл Игоревич", 0.75, 5.12, 2.85, C.graphite2);
    addChip(s, "Rust + PostgreSQL + Bitcoin Core", 3.78, 5.12, 3.15, C.graphite2);
    s.addText("01.03.02 Прикладная математика и информатика", {
      x: 9.35,
      y: 5.82,
      w: 2.8,
      h: 0.4,
      fontFace: F.body,
      fontSize: 11.5,
      color: C.muted,
      margin: 0,
      align: "right",
      fit: "shrink",
    });
    addNotes(s, notes[0]);
  }

  // 2
  {
    const s = pptx.addSlide();
    addBg(s, 2);
    addTitle(s, "01 / постановка", "Актуальность, цель и задачи ВКР", "Bitcoin Core остается источником, индексер формирует прикладный слой");
    s.addText("Актуальность", {
      x: 0.86,
      y: 2.1,
      w: 2.6,
      h: 0.26,
      fontFace: F.mono,
      fontSize: 10,
      bold: true,
      color: C.coral,
      margin: 0,
    });
    addBigClaim(s, "Прикладным сервисам\nнужен устойчивый слой\nструктурированных\nпредставлений", 0.82, 2.43, 5.35, 1.76, 27);
    s.addText("UTXO, балансы, mempool и reorg требуют нормализации поверх Bitcoin Core RPC.", {
      x: 0.86,
      y: 4.17,
      w: 4.95,
      h: 0.26,
      fontFace: F.body,
      fontSize: 12.8,
      color: C.muted,
      margin: 0,
      fit: "shrink",
    });
    s.addText("Цель ВКР", {
      x: 0.86,
      y: 4.62,
      w: 1.5,
      h: 0.24,
      fontFace: F.mono,
      fontSize: 10,
      bold: true,
      color: C.amber,
      margin: 0,
    });
    s.addText("Проектирование и реализация системы индексации блокчейн данных, обеспечивающей согласованное хранение и прикладную выдачу данных сети Bitcoin.", {
      x: 0.86,
      y: 4.98,
      w: 5.05,
      h: 0.98,
      fontFace: F.body,
      fontSize: 15.2,
      color: C.text,
      margin: 0,
      fit: "shrink",
      breakLine: false,
    });
    s.addText("Задачи ВКР", {
      x: 6.55,
      y: 2.1,
      w: 2.6,
      h: 0.28,
      fontFace: F.mono,
      fontSize: 10,
      bold: true,
      color: C.coral,
      margin: 0,
    });
    addNumberedTasks(s, [
      "анализ Bitcoin: UTXO, mempool, reorg",
      "требования к API, jobs и надежности",
      "архитектура: Bitcoin Core + PostgreSQL",
      "модель блоков, UTXO и балансов",
      "реализация RPC, indexer, mempool, reorg",
      "проверка функций и производительности",
    ], 6.55, 2.52, { gap: 0.56, fontSize: 13.4, textW: 4.82, textH: 0.32 });
    addNotes(s, notes[1]);
  }

  // 3
  {
    const s = pptx.addSlide();
    addBg(s, 3);
    addTitle(s, "02 / анализ", "Сравнение с существующими подходами", "Существующие решения не обеспечивают полного достижения цели и задач ВКР");
    const rows = [
      ["Подход", "Преимущества", "Недостатки"],
      ["Bitcoin Core RPC", "канонический источник блоков и транзакций", "нет готовых агрегатов и управления заданиями"],
      ["electrs / Esplora", "быстрые адресные запросы и explorer API", "иная модель хранения, фокус на обозревателе"],
      ["BlockSci", "аналитическая модель для исследований", "не является оперативным REST backend"],
      ["Bitcoin ETL", "пакетная выгрузка и подготовка датасетов", "нет постоянного API и жизненного цикла jobs"],
      ["Публичные API", "быстрый старт без собственного узла", "зависимость от внешнего сервиса и лимитов"],
      ["Результат ВКР", "согласованное хранение в PostgreSQL, прикладной REST API и управляемая индексация", "MVP: часть e2e/security-проверок требует развития"],
    ];
    const y0 = 2.02;
    rows.forEach((r, i) => {
      const y = y0 + i * 0.58;
      const isHeader = i === 0;
      const isWork = i === rows.length - 1;
      const fill = isHeader ? C.red2 : isWork ? C.graphite : i % 2 ? C.ink2 : C.graphite;
      s.addShape("roundRect", { x: 0.75, y, w: 11.95, h: isHeader ? 0.42 : 0.46, rectRadius: 0.02, fill: { color: fill, transparency: isHeader ? 0 : 12 }, line: { color: isWork ? C.coral : C.graphite2, transparency: 24, width: 0.75 } });
      s.addText(r[0], { x: 0.96, y: y + 0.11, w: 2.06, h: 0.2, fontFace: F.title, fontSize: isHeader ? 11.5 : 10.8, bold: true, color: C.text, margin: 0, fit: "shrink" });
      s.addText(r[1], { x: 3.35, y: y + 0.08, w: 4.12, h: 0.28, fontFace: F.body, fontSize: isHeader ? 11.5 : 10.2, bold: isHeader, color: isHeader ? C.text : C.muted, margin: 0, fit: "shrink" });
      s.addText(r[2], { x: 7.85, y: y + 0.11, w: 4.15, h: 0.2, fontFace: F.body, fontSize: isHeader ? 11.5 : 10.6, bold: isHeader, color: isWork ? C.text : C.muted, margin: 0, fit: "shrink" });
    });
    s.addText("Вывод: разработка собственного индексирующего слоя обоснована, поскольку существующие подходы не обеспечивают одновременно согласованное хранение, прикладную выдачу данных и управляемость обработки.", {
      x: 0.9,
      y: 6.22,
      w: 11.4,
      h: 0.42,
      fontFace: F.body,
      fontSize: 13.8,
      color: C.text,
      margin: 0,
      fit: "shrink",
    });
    addNotes(s, notes[2]);
  }

  // 4
  {
    const s = pptx.addSlide();
    addBg(s, 4);
    addTitle(s, "03 / требования", "Требования к проектируемой системе", "Функциональные и нефункциональные критерии");
    addNode(s, "Функциональные требования", "индексация блоков, транзакций, входов и выходов\nподдержка UTXO, адресных балансов и mempool\nуправление заданиями индексирования\nвыдача данных через REST API", 0.92, 2.28, 5.35, 2.62, { accent: C.coral, fill: C.graphite, titleSize: 20, subtitleSize: 13.2 });
    addNode(s, "Нефункциональные требования", "производительность API: p95 не более 100 мс\nадресные запросы: p95 до 45 мс\nпропускная способность: не ниже 29 RPS\nнадежность API: успешность не ниже 99,9%\nнаблюдаемость: p95, RPS, OK-rate, ошибки", 6.72, 2.18, 5.35, 2.98, { accent: C.steel, fill: C.ink2, titleSize: 19, subtitleSize: 12.1 });
    s.addText("Ключевое следствие: качество системы оценивается серией замеров и метрик, а не единичным пользовательским действием.", {
      x: 0.95,
      y: 5.82,
      w: 10.6,
      h: 0.38,
      fontFace: F.title,
      fontSize: 20,
      bold: true,
      color: C.text,
      margin: 0,
      fit: "shrink",
    });
    addNotes(s, notes[3]);
  }

  // 5
  {
    const s = pptx.addSlide();
    addBg(s, 5);
    addTitle(s, "04 / архитектура", "Архитектура решения и стек", "Stateless модульный монолит с внешним состоянием");
    addArchitectureDiagram(s);
    addNotes(s, notes[4]);
  }

  // 6
  {
    const s = pptx.addSlide();
    addBg(s, 6);
    addTitle(s, "05 / модель", "Модель данных и алгоритм индексации", "Обобщенная схема обработки блокчейн данных");
    addIndexingDiagram(s);
    addNotes(s, notes[5]);
  }

  // 7
  {
    const s = pptx.addSlide();
    addBg(s, 7);
    addTitle(s, "06 / балансы", "Индексация UTXO и балансов", "Баланс адреса строится из изменений входов и выходов");
    addBalanceIndexingDiagram(s);
    addNotes(s, notes[6]);
  }

  // 8
  {
    const s = pptx.addSlide();
    addBg(s, 8);
    addTitle(s, "07 / fork", "Откат состояния при fork/reorg", "Новый канонический хэш приводит к пересборке производных данных");
    addForkRollbackDiagram(s);
    addNotes(s, notes[7]);
  }

  // 9
  {
    const s = pptx.addSlide();
    addBg(s, 9);
    addTitle(s, "08 / качество", "Тестирование бизнес-функций", "Проверялись пользовательские сценарии и состояние данных");
    const rows = [
      ["Бизнес-функция", "Как проверено", "Достигнутый результат"],
      ["Управление jobs", "list/create/start/pause/resume/stop, auth и invalid transition", "200/201/401/404/409 согласно сценарию"],
      ["Выдача данных", "balance, history, UTXO, transactions, mempool, blocks через REST API", "JSON содержит ожидаемые totals, адреса и суммы"],
      ["Индексация блоков", "2 блока: coinbase 50 BTC и spend на addr1/addr2", "UTXO и балансы: 20 BTC + 30 BTC"],
      ["Mempool", "mock RPC: новая транзакция, затем исчезновение из mempool", "status: mempool -> dropped; фильтр по адресу работает"],
      ["Fork/reorg", "mock RPC возвращает другой hash на высоте 1", "старый хвост orphaned, баланс rebuilt до 50 BTC"],
    ];
    const widths = [2.45, 5.08, 3.86];
    const x0 = 0.75;
    const y0 = 2.0;
    rows.forEach((r, i) => {
      const y = i === 0 ? y0 : y0 + 0.48 + (i - 1) * 0.64;
      const h = i === 0 ? 0.42 : 0.56;
      const fill = i === 0 ? C.red2 : i % 2 ? C.ink2 : C.graphite;
      s.addShape("roundRect", { x: x0, y, w: 11.95, h, rectRadius: 0.018, fill: { color: fill, transparency: i === 0 ? 0 : 10 }, line: { color: C.graphite2, transparency: 28, width: 0.7 } });
      s.addText(r[0], { x: x0 + 0.17, y: y + 0.1, w: widths[0], h: 0.23, fontFace: F.title, fontSize: i === 0 ? 10.5 : 10.2, bold: true, color: C.text, margin: 0, fit: "shrink" });
      s.addText(r[1], { x: x0 + 2.82, y: y + 0.09, w: widths[1], h: 0.27, fontFace: F.body, fontSize: i === 0 ? 10.5 : 9.4, bold: i === 0, color: i === 0 ? C.text : C.muted, margin: 0, fit: "shrink" });
      s.addText(r[2], { x: x0 + 8.08, y: y + 0.09, w: widths[2], h: 0.27, fontFace: F.body, fontSize: i === 0 ? 10.5 : 9.5, bold: i === 0, color: i === 0 ? C.text : C.muted, margin: 0, fit: "shrink" });
    });
    s.addText("Вывод: проверка покрывает бизнес-контур индексирования, прикладную выдачу данных и восстановление согласованного состояния после fork.", {
      x: 0.9,
      y: 5.92,
      w: 11.3,
      h: 0.36,
      fontFace: F.title,
      fontSize: 18.5,
      color: C.text,
      bold: true,
      margin: 0,
      fit: "shrink",
    });
    addNotes(s, notes[8]);
  }

  // 10
  {
    const s = pptx.addSlide();
    addBg(s, 10);
    addTitle(s, "09 / производительность", "Замеры производительности", "Сравнение Bitcoin Core RPC и подготовленного PostgreSQL-индекса");
    const perfRows = [
      ["Код", "Сценарий", "RPC p95, мс", "Индексер p95, мс", "RPS индексера", "Ускорение p95"],
      ["C1", "состояние цепи", "85", "18", "125", "4,7x"],
      ["C2", "блок по хэшу", "480", "82", "29", "5,9x"],
      ["C3", "транзакция по txid", "230", "27", "83", "8,5x"],
      ["C4", "UTXO адреса", "420000", "45", "55", "9333x"],
      ["C5", "баланс адреса", "120000", "24", "111", "5000x"],
      ["C6", "mempool по адресу", "9000", "55", "50", "164x"],
    ];
    const px = 0.72;
    const py = 2.02;
    const col = [0.58, 2.88, 1.45, 1.72, 1.42, 1.42];
    perfRows.forEach((r, i) => {
      const y = py + i * 0.48;
      const fill = i === 0 ? C.red2 : i % 2 ? C.ink2 : C.graphite;
      s.addShape("roundRect", { x: px, y, w: 11.85, h: 0.39, rectRadius: 0.018, fill: { color: fill, transparency: i === 0 ? 0 : 10 }, line: { color: C.graphite2, transparency: 30, width: 0.7 } });
      let x = px + 0.14;
      r.forEach((text, j) => {
        s.addText(text, {
          x,
          y: y + 0.09,
          w: col[j],
          h: 0.2,
          fontFace: j === 1 || i === 0 ? F.body : F.mono,
          fontSize: i === 0 ? 9.3 : 9.6,
          bold: i === 0 || j === 0 || j === 5,
          color: i === 0 ? C.text : j === 5 ? C.coral : C.muted,
          margin: 0,
          fit: "shrink",
        });
        x += col[j] + 0.24;
      });
    });
    addMetric(s, "50+1000", "warm-up и измеряемые\nзапросы на сценарий", 0.88, 5.72, C.dim);
    addMetric(s, "99,9%", "успешность API\nиндексера", 2.98, 5.72, C.green);
    addMetric(s, "C4/C5", "максимальный выигрыш:\nUTXO и балансы", 5.08, 5.72, C.amber);
    s.addText("Вывод: заранее построенный индекс снижает задержку там, где RPC выполняет сканирование или возвращает данные для клиентской фильтрации.", {
      x: 7.35,
      y: 5.82,
      w: 4.75,
      h: 0.52,
      fontFace: F.title,
      fontSize: 16.2,
      bold: true,
      color: C.text,
      margin: 0,
      fit: "shrink",
    });
    addNotes(s, notes[9]);
  }

  // 11
  {
    const s = pptx.addSlide();
    addBg(s, 11);
    addTitle(s, "10 / результат", "Результаты работы", "Количество результатов соответствует количеству задач ВКР");
    s.addText("Bitcoin Blockchain Indexer", { x: 0.95, y: 2.22, w: 5.1, h: 0.52, fontFace: F.title, fontSize: 28, bold: true, color: C.text, margin: 0, fit: "shrink" });
    addShortList(s, [
      { text: "предметная область и аналоги проанализированы" },
      { text: "требования к системе сформированы" },
      { text: "архитектура stateless модульного монолита обоснована" },
      { text: "модель данных PostgreSQL спроектирована" },
      { text: "ключевые модули индексатора и API реализованы" },
      { text: "качество проверено бизнес-сценариями" },
    ], 0.98, 3.0, 6.1, 0.38, { fontSize: 14.1, textH: 0.27 });
    s.addText("github.com/vivichv9/Blockchain-Indexer", { x: 0.98, y: 5.72, w: 5.6, h: 0.28, fontFace: F.mono, fontSize: 11.5, color: C.amber, margin: 0, fit: "shrink" });
    s.addShape("roundRect", { x: 8.15, y: 1.75, w: 3.25, h: 3.25, rectRadius: 0.03, fill: { color: "FFFFFF" }, line: { color: C.red, width: 1.2 } });
    s.addImage({ path: assets.qr, x: 8.36, y: 1.96, w: 2.82, h: 2.82, hyperlink: { url: "https://github.com/vivichv9/Blockchain-Indexer" } });
    addChip(s, "QR на GitHub", 8.55, 5.35, 1.65, C.red2);
    addNotes(s, notes[10]);
  }

  // 12
  {
    const s = pptx.addSlide();
    addBg(s, 12);
    addTitle(s, "11 / значимость", "Практическая значимость проекта", "Прикладной слой данных поверх Bitcoin Core");
    addBigClaim(s, "Нормализованные\nданные вместо\nнизкоуровневых\nRPC-ответов", 0.85, 2.25, 5.3, 1.9, 29);
    const uses = [
      ["analytics", "исследование и мониторинг"],
      ["explorer backend", "выдача блоков и транзакций"],
      ["wallet services", "UTXO и адресные балансы"],
      ["operations", "jobs, logs, metrics"],
    ];
    uses.forEach((u, i) => {
      const x = 6.85;
      const y = 2.08 + i * 0.82;
      s.addText(u[0].toUpperCase(), { x, y, w: 2.2, h: 0.22, fontFace: F.mono, fontSize: 9.5, bold: true, color: i === 0 ? C.coral : C.steel, margin: 0, fit: "shrink" });
      s.addText(u[1], { x: 9.0, y: y - 0.03, w: 3.0, h: 0.26, fontFace: F.body, fontSize: 14, color: C.text, margin: 0, fit: "shrink" });
      s.addShape("line", { x: 6.85, y: y + 0.34, w: 4.8, h: 0, line: { color: C.graphite2, width: 1 } });
    });
    s.addText("Дальше: полный regtest e2e, усиление security-контура, расширение метрик, оптимизация запросов.", {
      x: 0.9,
      y: 5.92,
      w: 9.6,
      h: 0.36,
      fontFace: F.title,
      fontSize: 20,
      color: C.text,
      bold: true,
      margin: 0,
      fit: "shrink",
    });
    addNotes(s, notes[11]);
  }

  await pptx.writeFile({ fileName: PPTX_PATH });
}

async function previewSlidePng(index, title, subtitle, bullets, imagePath = null) {
  const width = 1600;
  const height = 900;
  let image = "";
  if (imagePath && fs.existsSync(imagePath)) {
    const data = await imageDataUri(imagePath);
    image = `<rect x="935" y="230" width="520" height="390" rx="12" fill="#FFFFFF" stroke="#D8C7AE"/>
<image href="${data}" x="955" y="250" width="480" height="330" preserveAspectRatio="xMidYMid meet"/>`;
  }
  const lineText = wrapText(title, 28).slice(0, 2);
  const titleSvg = lineText.map((l, i) => `<text x="86" y="${140 + i * 52}" font-family="${F.title}" font-size="46" font-weight="700" fill="#${C.text}">${escapeXml(l)}</text>`).join("");
  const bulletsSvg = bullets.map((b, i) => {
    const y = 340 + i * 62;
    return `<line x1="94" y1="${y - 9}" x2="145" y2="${y - 9}" stroke="#${i === 0 ? C.coral : C.steel}" stroke-width="5"/>
<text x="168" y="${y}" font-family="${F.body}" font-size="27" fill="#${C.text}">${escapeXml(b)}</text>`;
  }).join("");
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="0 0 ${width} ${height}">
<rect width="1600" height="900" fill="#${C.ink}"/>
<polygon points="-120,0 500,0 320,900 -120,900" fill="#${index % 3 === 0 ? C.red2 : C.graphite}" opacity="0.72"/>
<polygon points="1180,0 1600,0 1600,900 1035,900" fill="#${index % 2 === 0 ? C.graphite2 : C.red2}" opacity="0.45"/>
${Array.from({ length: 10 }, (_, i) => `<line x1="${90 + i * 145}" y1="0" x2="${90 + i * 145}" y2="900" stroke="#B6A68F" opacity="0.18"/>`).join("")}
<line x1="72" y1="52" x2="1520" y2="52" stroke="#${C.red}" stroke-width="4"/>
<text x="86" y="105" font-family="${F.mono}" font-size="18" font-weight="700" fill="#${C.coral}">SLIDE ${String(index).padStart(2, "0")}</text>
${titleSvg}
<text x="88" y="256" font-family="${F.body}" font-size="25" fill="#${C.muted}">${escapeXml(subtitle || "")}</text>
${bulletsSvg}
${image}
<line x1="72" y1="844" x2="1520" y2="844" stroke="#${C.graphite2}" stroke-width="2"/>
<text x="86" y="872" font-family="${F.mono}" font-size="15" fill="#${C.dim}">BITCOIN BLOCKCHAIN INDEXER</text>
</svg>`;
  const out = path.join(PREVIEW_DIR, `slide-${String(index).padStart(2, "0")}.png`);
  await sharp(Buffer.from(svg)).png().toFile(out);
  return out;
}

async function buildPreviews() {
  const previewSpecs = [
    ["Система индексации блокчейн данных", "ВКР бакалавра / 7 минут", ["Bitcoin Blockchain Indexer", "Rust + PostgreSQL + Bitcoin Core", "Ефименко Кирилл Игоревич"]],
    ["Актуальность, цель и задачи ВКР", "Bitcoin Core как источник, индексер как прикладный слой", ["UTXO, балансы, mempool, reorg", "6 задач из введения", "проверка функций и производительности"]],
    ["Сравнение с существующими подходами", "Преимущества и недостатки решений", ["нет полного достижения цели и задач ВКР", "преимущества и ограничения", "результат ВКР: хранение, API, jobs"]],
    ["Требования к проектируемой системе", "Функциональные и нефункциональные критерии", ["p95 API: до 100 мс", "RPS: не ниже 29", "OK-rate: 99,9%"]],
    ["Архитектура решения и стек", "Роли компонентов архитектуры", ["Bitcoin Core RPC", "Backend: индексация / jobs / REST API", "PostgreSQL"]],
    ["Модель данных и алгоритм индексации", "Обобщенная схема обработки", ["диапазон обработки", "проверка reorg", "нормализация и фиксация"]],
    ["Индексация UTXO и балансов", "Изменения входов и выходов", ["выходы: +value_sats", "входы: -value_sats", "history snapshot"]],
    ["Откат состояния при fork/reorg", "Пересборка производных данных", ["hash divergence", "orphaned tail", "replay canonical"]],
    ["Тестирование бизнес-функций", "Сценарии и достигнутые результаты", ["jobs / data API", "pipeline / mempool", "fork / reorg"]],
    ["Замеры производительности", "RPC против PostgreSQL-индекса", ["p95 задержка", "API OK: 99,9%", "ускорение до 9333x"]],
    ["Результаты работы", "6 результатов на 6 задач ВКР", ["анализ и требования", "архитектура и модель", "реализация и проверка"], assets.qr],
    ["Практическая значимость проекта", "Прикладной слой данных поверх Bitcoin Core", ["нормализованные ответы", "analytics / explorer / wallet", "развитие: e2e, security, metrics"]],
  ];
  const out = [];
  for (let i = 0; i < previewSpecs.length; i += 1) {
    out.push(await previewSlidePng(i + 1, ...previewSpecs[i]));
  }
  const thumbs = await Promise.all(out.map((p) => sharp(p).resize(400, 225).toBuffer()));
  const cols = 3;
  const thumbW = 400;
  const thumbH = 225;
  const rows = Math.ceil(thumbs.length / cols);
  const sheet = sharp({
    create: {
      width: cols * thumbW,
      height: rows * thumbH,
      channels: 4,
      background: hexToRgb(C.ink).concat([1]),
    },
  });
  const composite = thumbs.map((input, i) => ({
    input,
    left: (i % cols) * thumbW,
    top: Math.floor(i / cols) * thumbH,
  }));
  await sheet.composite(composite).png().toFile(path.join(PREVIEW_DIR, "contact-sheet.png"));
}

async function main() {
  for (const [name, p] of Object.entries(assets)) {
    if (!fs.existsSync(p)) throw new Error(`Missing asset ${name}: ${p}`);
  }
  await buildPptx();
  await buildPreviews();
  console.log(`PPTX: ${PPTX_PATH}`);
  console.log(`PREVIEWS: ${PREVIEW_DIR}`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
