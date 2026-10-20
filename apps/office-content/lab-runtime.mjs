const javascriptLabMounts = new WeakMap();
let javascriptLabSerial = 0;

export function mountJavascriptLab(root, payload) {
  const kinds = ["simulator", "network", "priority", "playback", "comparison", "quiz", "explorer", "canvas"];
  const plain = payload && [Object.prototype, null].includes(Object.getPrototypeOf(payload));
  const keys = plain ? Reflect.ownKeys(payload) : [];
  const expected = ["version", "kind", "title", "source"];
  if (!plain || ![4, 5].includes(keys.length) || keys.some(key => ![...expected, "canvas"].includes(key))
    || expected.some(key => !Object.hasOwn(payload, key))
    || keys.some(key => !Object.hasOwn(Object.getOwnPropertyDescriptor(payload, key), "value"))) {
    throw new TypeError("ラボ設定は version・kind・title・source と任意の canvas を持つ通常のレコードにしてください。");
  }
  const { version, kind, title, source } = payload;
  if (version !== 1 || !kinds.includes(kind) || typeof title !== "string" || !title.trim()
    || Array.from(title).length > 120 || typeof source !== "string" || Array.from(source).length > 240) {
    throw new TypeError("ラボ設定が不正です。version は 1、kind は対応する8種類、title は1〜120文字、source は240文字以内です。");
  }
  if (!root || root.nodeType !== 1 || !root.ownerDocument?.defaultView || typeof root.replaceChildren !== "function") {
    throw new TypeError("ラボの取り付け先にはブラウザーの要素を指定してください。");
  }
  const canvasSize = Object.hasOwn(payload, "canvas") ? payload.canvas : { width: 1280, height: 720 };
  if (!canvasSize || ![Object.prototype, null].includes(Object.getPrototypeOf(canvasSize))
    || Reflect.ownKeys(canvasSize).length !== 2 || !["width", "height"].every(key =>
      Object.hasOwn(Object.getOwnPropertyDescriptor(canvasSize, key) ?? {}, "value")
      && Number.isFinite(canvasSize[key]) && canvasSize[key] >= (key === "width" ? 960 : 480) && canvasSize[key] <= 4096)) {
    throw new TypeError("canvas は実際のスライド寸法（幅960以上・高さ480以上、各4096以下）を指定してください。");
  }
  javascriptLabMounts.get(root)?.dispose();
  const doc = root.ownerDocument, win = doc.defaultView, id = `js-lab-${++javascriptLabSerial}`;
  const cleanups = [], sceneCleanups = [];
  let disposed = false, state = {}, resizeCanvasScene = null;
  const hadClass = root.classList.contains("js-lab-root");
  const el = (tag, className = "", text) => {
    const node = doc.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };
  const svg = (tag, attrs = {}, text) => {
    const node = doc.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, String(value));
    if (text !== undefined) node.textContent = text;
    return node;
  };
  const on = (node, name, handler, options, scope = sceneCleanups) => {
    node.addEventListener(name, handler, options);
    scope.push(() => node.removeEventListener(name, handler, options));
  };
  const button = (text, handler, className = "") => {
    const node = el("button", `lab-button ${className}`, text);
    node.type = "button";
    if (handler) on(node, "click", handler);
    return node;
  };
  const marked = (tag, testId, text = "") => {
    const node = el(tag, "", text);
    node.dataset.testid = testId;
    return node;
  };
  const clamp = (value, min = 0, max = 100) => Math.max(min, Math.min(max, Number.isFinite(value) ? value : min));
  const range = (parent, label, min, max, value, change) => {
    const wrap = el("label", "lab-field"), top = el("span", "lab-field-label", label);
    const output = el("output", "lab-field-value", String(value)), input = el("input");
    input.type = "range"; input.min = String(min); input.max = String(max); input.step = "1"; input.value = String(value);
    input.setAttribute("aria-label", label);
    top.append(output); wrap.append(top, input); parent.append(wrap);
    on(input, "input", () => {
      const next = clamp(Number(input.value), min, max);
      output.textContent = String(next); change(next);
    });
    return { input, set(next) { input.value = String(next); output.textContent = String(Math.round(next)); } };
  };
  const live = (testId, text = "") => {
    const node = marked("output", testId, text);
    node.className = "lab-detail"; node.setAttribute("aria-live", "polite");
    return node;
  };
  const pointer = (node, surface, update, start = () => {}) => {
    let pointerId = null;
    const move = event => {
      const box = surface.getBoundingClientRect();
      if (box.width && box.height) update(clamp((event.clientX - box.left) / box.width * 100), clamp((event.clientY - box.top) / box.height * 100));
    };
    const release = () => {
      const active = pointerId; pointerId = null;
      if (active !== null && node.hasPointerCapture?.(active)) node.releasePointerCapture(active);
    };
    on(node, "pointerdown", event => {
      if (pointerId !== null || event.button !== 0 || event.isPrimary === false) return;
      pointerId = event.pointerId; start();
      node.setPointerCapture?.(pointerId);
      move(event);
    });
    on(node, "pointermove", event => {
      if (event.pointerId !== pointerId) return;
      event.preventDefault(); move(event);
    });
    on(node, "pointerup", release); on(node, "pointercancel", release);
    on(node, "lostpointercapture", () => { pointerId = null; });
    sceneCleanups.push(release);
  };
  const money = value => `${value.toLocaleString("ja-JP")}円`;
  const motion = win.matchMedia?.("(prefers-reduced-motion: reduce)");
  root.classList.add("js-lab-root");
  const shell = el("div", "lab-shell"), header = el("header", "lab-header");
  const heading = el("div"), eyebrow = el("p", "lab-eyebrow", "JAVASCRIPT / 触って確かめる");
  const main = el("main", "lab-main"), footer = el("footer", "lab-footer");
  const explanation = el("section", "lab-explanation");
  explanation.hidden = true;
  const explanationToggle = el("button", "lab-button lab-reset", "解説");
  explanationToggle.type = "button";
  explanationToggle.setAttribute("aria-expanded", "false");
  explanation.id = `${id}-explanation`; explanationToggle.setAttribute("aria-controls", explanation.id);
  const paneToggle = el("button", "lab-button lab-reset lab-pane-toggle", "操作を表示");
  paneToggle.type = "button"; paneToggle.setAttribute("aria-pressed", "false");
  let currentVisual, currentControls;
  const longTitle = Array.from(title).length > 36;
  heading.append(eyebrow, el("h1", "", longTitle ? "JavaScript 表現実験" : title));
  if (longTitle) eyebrow.textContent = "見出し全文は「解説」に表示";
  const reset = el("button", "lab-button lab-reset", "リセット"); reset.type = "button";
  const headerActions = el("div", "lab-header-actions");
  headerActions.append(paneToggle, explanationToggle, reset); header.append(heading, headerActions);
  footer.append(el("strong", "", "架空データ・説明用"), el("span", "", "入力は保存・送信しません。前提と式は「解説」。"));
  shell.style.width = `${canvasSize.width}px`; shell.style.height = `${canvasSize.height}px`;
  shell.style.containerType = "size";
  root.dataset.canvasWidth = String(canvasSize.width); root.dataset.canvasHeight = String(canvasSize.height);
  shell.append(header, main, explanation, footer); root.replaceChildren(shell);
  const fitCanvas = () => {
    if (disposed) return;
    const box = root.getBoundingClientRect();
    const scale = Math.min(box.width / canvasSize.width, box.height / canvasSize.height);
    shell.style.transform = `scale(${scale})`;
    shell.style.left = `${(box.width - canvasSize.width * scale) / 2}px`;
    shell.style.top = `${(box.height - canvasSize.height * scale) / 2}px`;
    resizeCanvasScene?.();
  };
  const observer = new win.ResizeObserver(fitCanvas); observer.observe(root); cleanups.push(() => observer.disconnect());
  fitCanvas();
  let explanationPages = [], explanationIndex = 0;
  const explanationBody = el("div"), explanationActions = el("nav", "lab-explanation-actions");
  const explanationPosition = el("output");
  explanationActions.setAttribute("aria-label", "解説のページ");
  const explanationBack = el("button", "lab-button", "前の解説"), explanationNext = el("button", "lab-button", "次の解説");
  explanationBack.type = explanationNext.type = "button";
  explanationActions.append(explanationBack, explanationPosition, explanationNext);
  explanation.append(explanationBody, explanationActions);
  const showExplanation = () => {
    explanationBody.replaceChildren(explanationPages[explanationIndex]);
    explanationPosition.textContent = `${explanationIndex + 1} / ${explanationPages.length}`;
    explanationBack.disabled = explanationIndex === 0;
    explanationNext.disabled = explanationIndex === explanationPages.length - 1;
  };
  const explanationPage = (label, ...contents) => {
    const section = el("section", "lab-explanation-page");
    section.append(el("h2", "", label), ...contents.map(value => el("p", "", value)));
    explanationPages.push(section);
  };
  const card = (headline, description, concept, limit) => {
    const section = el("section", "lab-card"), intro = el("div", "lab-intro");
    intro.append(el("h2", "", headline));
    const grid = el("div", "lab-grid"), visual = el("div", "lab-visual"), controls = el("div", "lab-controls");
    currentVisual = visual; currentControls = controls;
    shell.dataset.pane = "visual"; paneToggle.textContent = "操作を表示"; paneToggle.setAttribute("aria-pressed", "false");
    grid.append(visual, controls);
    const note = el("p", "lab-concept", limit);
    section.append(intro, grid, note); main.append(section);
    explanationPages = []; explanationIndex = 0;
    explanationPage(title, description, `表現の仕組み：${concept}`);
    explanationPage("条件・データと出典", limit, source);
    showExplanation();
    explanationToggle.textContent = "解説"; explanationToggle.setAttribute("aria-expanded", "false");
    explanation.hidden = true; main.hidden = false;
    return { visual, controls };
  };
  const animation = (advance, changed) => {
    let request = null, previous = null;
    const stop = () => {
      if (request !== null) win.cancelAnimationFrame(request);
      request = null; previous = null;
      if (state.running) { state.running = false; changed(); }
    };
    const tick = timestamp => {
      request = null;
      if (disposed || !shell.isConnected || doc.hidden || motion?.matches) { stop(); return; }
      const delta = previous === null ? 0 : Math.min((timestamp - previous) / 1000, 0.1);
      previous = timestamp;
      if (!advance(delta)) { stop(); return; }
      request = win.requestAnimationFrame(tick);
    };
    const start = () => {
      if (state.running || doc.hidden || motion?.matches || !shell.isConnected) return;
      state.running = true; previous = null; changed(); request = win.requestAnimationFrame(tick);
    };
    on(doc, "visibilitychange", () => { if (doc.hidden) stop(); });
    if (motion?.addEventListener) on(motion, "change", () => { if (motion.matches) stop(); });
    sceneCleanups.push(stop);
    return { start, stop };
  };
  const scenes = {
    simulator() {
      state = { units: 80, rides: 4, price: 300 };
      const { visual, controls } = card("条件を変えると、収支はどう動く？", "3つの条件を連続的に変え、同じ計算式でその場で再計算します。", "入力 → 数式 → 表示の更新", "単純な仮定・予測ではない");
      const metrics = el("div", "lab-metrics"), bars = el("div", "lab-bars");
      const outputs = {}, fills = {};
      for (const [key, label] of [["revenue", "売上"], ["cost", "費用"], ["balance", "収支"]]) {
        const metric = el("div", "lab-metric");
        outputs[key] = marked("output", key); metric.append(el("span", "", label), outputs[key]); metrics.append(metric);
        const row = el("div", "lab-bar-row"), track = el("div", "lab-bar-track"), fill = el("div", `lab-bar lab-${key}`);
        track.setAttribute("role", "img");
        fills[key] = fill; track.append(fill); row.append(el("span", "", label), track); bars.append(row);
      }
      const formula = "売上 = 台数 × 利用回数 × 料金\n費用 = 台数 × 500円 + 20,000円\n収支 = 売上 − 費用";
      visual.append(metrics, bars);
      explanationPage("計算式と前提", formula, "単純な説明用の計算です。需要変動・税・稼働率は含みません。");
      showExplanation();
      const update = () => {
        state.revenue = state.units * state.rides * state.price;
        state.cost = state.units * 500 + 20000; state.balance = state.revenue - state.cost;
        const scale = Math.max(state.revenue, state.cost, Math.abs(state.balance), 1);
        for (const key of Object.keys(outputs)) {
          outputs[key].textContent = money(state[key]);
          fills[key].style.width = `${Math.abs(state[key]) / scale * 100}%`;
          fills[key].dataset.negative = String(state[key] < 0);
          fills[key].parentNode.setAttribute("aria-label", `${outputs[key].previousSibling.textContent} ${money(state[key])}`);
        }
      };
      range(controls, "台数", 20, 200, 80, value => { state.units = value; update(); });
      range(controls, "1台あたりの利用回数", 1, 10, 4, value => { state.rides = value; update(); });
      range(controls, "1回の料金", 100, 1000, 300, value => { state.price = value; update(); });
      controls.append(el("p", "lab-note", "費用は説明用の固定モデルです。需要変動・税・稼働率は含みません。"));
      metrics.setAttribute("aria-live", "polite"); update();
    },
    network() {
      state = { selected: "entry", failure: null };
      const { visual, controls } = card("つながりを選んで、影響をたどる", "ノードを押して役割を確認。選んだ場所の障害を切り替えると、後続への影響が変わります。", "グラフの探索と依存関係の計算", "架空の業務フロー・実システムの診断ではない");
      const nodes = [
        ["entry", "入口", 14, 50, "依頼を受け付ける場所。"],
        ["auth", "認証", 38, 50, "利用者を確認する場所。"],
        ["pay", "決済", 62, 24, "料金の処理を行う場所。"],
        ["notice", "通知", 86, 50, "処理結果を知らせる場所。"]
      ];
      const edges = [["entry", "auth"], ["auth", "pay"], ["pay", "notice"], ["auth", "notice"]];
      const board = el("div", "lab-network"), drawing = svg("svg", { viewBox: "0 0 100 100", preserveAspectRatio: "none", "aria-hidden": "true" });
      const paths = edges.map(([from, to]) => {
        const a = nodes.find(node => node[0] === from), b = nodes.find(node => node[0] === to);
        const line = svg("path", { d: `M ${a[2]} ${a[3]} L ${b[2]} ${b[3]}`, class: "lab-edge" }); drawing.append(line); return line;
      });
      board.append(drawing); visual.append(board, el("p", "lab-note", "実線：通常のつながり ／ 破線：障害による影響経路"));
      const detail = live("node-detail"), impact = live("network-impact");
      const toggle = button("障害を切り替える", () => { state.failure = state.failure === state.selected ? null : state.selected; update(); });
      controls.append(detail, toggle, impact);
      const buttons = nodes.map(node => {
        const target = button(node[1], () => { state.selected = node[0]; update(); }, "lab-node");
        target.dataset.node = node[0]; target.style.left = `${node[2]}%`; target.style.top = `${node[3]}%`;
        board.append(target); return target;
      });
      function update() {
        const affected = new Set(state.failure ? [state.failure] : []);
        for (let i = 0; i < nodes.length; i++) for (const [from, to] of edges) if (affected.has(from)) affected.add(to);
        nodes.forEach((node, index) => {
          buttons[index].setAttribute("aria-pressed", String(state.selected === node[0]));
          buttons[index].dataset.affected = String(affected.has(node[0]));
          buttons[index].dataset.failed = String(state.failure === node[0]);
        });
        paths.forEach((path, index) => { path.dataset.affected = String(affected.has(edges[index][0])); });
        const selected = nodes.find(node => node[0] === state.selected);
        const status = state.failure === state.selected ? "障害中" : affected.has(state.selected) ? "前段の障害の影響あり" : "通常";
        detail.textContent = `${selected[1]}：${selected[4]} 状態：${status}。`;
        impact.textContent = state.failure ? `障害：${nodes.find(node => node[0] === state.failure)[1]}。影響するノード：${nodes.filter(node => affected.has(node[0])).map(node => node[1]).join(" → ")}。` : "障害なし。4つのノードは通常状態です。";
      }
      update();
    },
    priority() {
      const tiles = [["予約画面", 25, 65], ["点検手順", 40, 40], ["地域連携", 65, 60], ["広域展開", 75, 25]];
      state = { selected: "予約画面", positions: tiles.map(([label, effort, impact]) => ({ label, effort, impact })) };
      const { visual, controls } = card("優先順位を、配置しながら話す", "施策をドラッグするか、選んで矢印キーで移動。右のスライダーでも位置を変更できます。", "ポインター操作・キー操作・座標の同期", "仮の定性的な位置・測定値ではない");
      const board = el("div", "lab-priority"), plane = el("div", "lab-priority-plane");
      board.append(el("span", "lab-axis-top", "期待する影響：高"), el("span", "lab-axis-bottom", "実行負担：小 → 大"), plane);
      visual.append(board);
      const help = el("p", "lab-note", "施策を選択し、ドラッグ／矢印キーで5ポイントずつ移動。スライダーは1ポイント単位です。");
      help.id = `${id}-priority-help`;
      const detail = live("priority-position"); controls.append(help, detail);
      const current = () => state.positions.find(item => item.label === state.selected);
      const effort = range(controls, "実行負担", 0, 100, 25, value => { current().effort = value; update(); });
      const impact = range(controls, "期待する影響", 0, 100, 65, value => { current().impact = value; update(); });
      const buttons = state.positions.map(item => {
        const target = button(item.label, () => { state.selected = item.label; update(); }, "lab-priority-tile");
        target.setAttribute("aria-describedby", help.id);
        pointer(target, plane, (x, y) => { item.effort = Math.round(x); item.impact = Math.round(100 - y); update(); }, () => { state.selected = item.label; update(); });
        on(target, "keydown", event => {
          const offset = { ArrowLeft: [-5, 0], ArrowRight: [5, 0], ArrowUp: [0, 5], ArrowDown: [0, -5] }[event.key];
          if (!offset) return;
          event.preventDefault(); state.selected = item.label;
          item.effort = clamp(item.effort + offset[0]); item.impact = clamp(item.impact + offset[1]); update();
        });
        plane.append(target); return target;
      });
      function update() {
        state.positions.forEach((item, index) => {
          const target = buttons[index]; target.style.left = `${item.effort}%`; target.style.top = `${100 - item.impact}%`;
          target.dataset.effort = String(item.effort); target.dataset.impact = String(item.impact);
          target.setAttribute("aria-pressed", String(item.label === state.selected));
        });
        effort.set(current().effort); impact.set(current().impact);
        detail.textContent = `${state.selected}：実行負担 ${current().effort} / 100、期待する影響 ${current().impact} / 100（仮置き）`;
      }
      update();
    },
    playback() {
      state = { time: 0, running: false };
      const { visual, controls } = card("流れを動かし、気になる瞬間で止める", "架空の配送を時間軸で観察。つまみを動かせば、任意の時点へ直接移れます。", "経過時間に基づくアニメーションとスクラブ", "約14秒の説明用再生・実際の所要時間ではない");
      const flow = el("div", "lab-flow"), track = el("div", "lab-flow-track"), marker = el("div", "lab-flow-marker", "荷物");
      const stages = ["受付", "仕分け", "輸送", "お届け"];
      const labels = el("div", "lab-flow-stages");
      stages.forEach(label => labels.append(el("span", "", label)));
      track.append(marker); flow.append(labels, track, el("p", "lab-note", "架空の時系列サンプル：進捗 0 → 100"));
      visual.append(flow);
      const progress = live("time-progress"), scrub = range(controls, "時点", 0, 100, 0, value => { state.time = value; update(); if (value === 100) player.stop(); });
      const play = button("再生", () => { if (state.running) player.stop(); else { if (motion?.matches) return; if (state.time >= 100) state.time = 0; player.start(); update(); } });
      const player = animation(delta => { state.time = Math.min(100, state.time + delta * 100 / 14); update(); return state.time < 100; }, update);
      controls.append(play, progress, el("p", "lab-note", "自動再生しません。別のタブに移ると停止します。動きを減らす設定では、つまみで時点を選べます。"));
      function update() {
        marker.style.left = `${state.time}%`; scrub.set(state.time);
        progress.textContent = `${Math.round(state.time)}% · ${stages[Math.min(3, Math.floor(state.time / 25))]}`;
        play.textContent = state.running ? "一時停止" : "再生";
        play.disabled = Boolean(motion?.matches);
      }
      if (motion?.addEventListener) on(motion, "change", update);
      update();
    },
    comparison() {
      state = { boundary: 50 };
      const { visual, controls } = card("境界を動かして、変更点を見比べる", "同じ領域に2つの画面案を重ねています。中央のハンドル、またはスライダーで表示範囲を変えられます。", "CSSのクリップ領域を連続的に変更", "架空のUIモック・実製品の画面ではない");
      const board = el("div", "lab-comparison");
      function panel(after) {
        const node = el("div", `lab-mock ${after ? "lab-mock-after" : "lab-mock-before"}`);
        node.append(el("h3", "", after ? "変更案" : "変更前"), el("p", "", after ? "配送の状況を、ひと目で" : "配送管理 / 詳細一覧"));
        const stats = el("div", "lab-mock-stats");
        for (const [label, value] of [["受付", "128"], ["配送中", "42"], ["確認待ち", "6"]]) {
          const tile = el("div"); tile.append(el("span", "", label), el("strong", "", value)); stats.append(tile);
        }
        const rows = el("div", "lab-mock-rows");
        ["中央エリア　配送中", "港エリア　確認待ち", "丘エリア　配送済み"].forEach(text => rows.append(el("p", "", text)));
        node.append(stats, rows, el("small", "", "説明用の架空データ")); return node;
      }
      const before = panel(false), after = panel(true); after.dataset.testid = "comparison-after";
      const line = el("div", "lab-comparison-line"), handle = button("↔", null, "lab-divider");
      handle.setAttribute("role", "slider"); handle.setAttribute("aria-label", "比較の境界");
      handle.setAttribute("aria-valuemin", "0"); handle.setAttribute("aria-valuemax", "100");
      board.append(before, after, line, handle); visual.append(board);
      const slider = range(controls, "境界位置", 0, 100, 50, value => { state.boundary = value; update(); });
      const detail = live("comparison-position"); controls.append(detail, el("p", "lab-note", "左：変更案 ／ 右：変更前。ハンドルは左右キーでも動かせます。"));
      pointer(handle, board, x => { state.boundary = Math.round(x); update(); });
      on(handle, "keydown", event => {
        if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
        event.preventDefault();
        state.boundary = event.key === "Home" ? 0 : event.key === "End" ? 100 : clamp(state.boundary + (event.key === "ArrowLeft" ? -1 : 1)); update();
      });
      function update() {
        after.style.clipPath = `inset(0 ${100 - state.boundary}% 0 0)`;
        line.style.left = `${state.boundary}%`; handle.style.left = `clamp(24px, ${state.boundary}%, calc(100% - 24px))`;
        handle.setAttribute("aria-valuenow", String(state.boundary)); slider.set(state.boundary);
        detail.textContent = `変更案 ${state.boundary}% ／ 変更前 ${100 - state.boundary}%`;
      }
      update();
    },
    quiz() {
      state = { question: 0, path: [] };
      const { visual, controls } = card("選択の先にある理由を考える", "新しい地域サービスの進め方を、2回の選択で考える練習です。正解率や点数はありません。", "選択によって分かれる状態と説明", "架空の意思決定演習・個別の助言ではない");
      const stage = el("div", "lab-quiz"), crumb = live("quiz-path"), result = live("quiz-result");
      const back = button("一つ戻る", () => { state.path.pop(); render(true); });
      visual.append(stage, result); controls.append(crumb, back);
      on(stage, "click", event => {
        const choice = event.target.closest?.("button[data-choice]");
        if (!choice || !stage.contains(choice)) return;
        state.path.push(choice.dataset.choice); render(true);
      });
      function render(focus = false) {
        state.question = state.path.length;
        stage.replaceChildren();
        const title = el("h3", "", state.question === 0 ? "まず、どの広げ方を選びますか？" : state.question === 1 ? "次の判断を、何に基づけますか？" : "選択した道筋を振り返る");
        title.tabIndex = -1; stage.append(el("p", "lab-eyebrow", `STEP ${Math.min(2, state.question + 1)} / 2`), title);
        const choices = state.question === 0 ? ["小さく試す", "一度に広げる"] : state.question === 1 ? ["観測して判断", "前提を再確認"] : [];
        const choiceBox = el("div", "lab-quiz-choices");
        for (const choice of choices) {
          const target = button(choice); target.dataset.choice = choice; choiceBox.append(target);
        }
        stage.append(choiceBox);
        crumb.textContent = `道筋：${state.path.length ? state.path.join(" → ") : "まだ選択していません"}`;
        back.disabled = state.question === 0; result.textContent = "";
        if (state.question === 2) {
          const small = state.path[0] === "小さく試す", observe = state.path[1] === "観測して判断";
          result.textContent = `${state.path.join(" → ")}。${small ? "対象を限定すると学習範囲を絞れますが、他地域への適用には検証が必要です。" : "広く始めると複数地域を比較できますが、対応負担と中止条件の準備が必要です。"}${observe ? "観測の前に、見る指標と判断する時点を決めておきましょう。" : "前提を確かめる際は、何が違えば計画を変えるかを明らかにしましょう。"} 架空の演習であり、優劣の採点ではありません。`;
        }
        if (focus) title.focus();
      }
      render();
    },
    explorer() {
      const records = [
        ["中央", "都市", 360, 82], ["港", "都市", 280, 76], ["丘", "郊外", 190, 88], ["川", "郊外", 240, 80],
        ["森", "郊外", 120, 91], ["駅前", "都市", 420, 79], ["海辺", "郊外", 160, 85], ["北町", "都市", 210, 84]
      ];
      state = { query: "", category: "全て", direction: "desc", selected: "中央", count: 8 };
      const { visual, controls } = card("表を絞り込み、気になる地域を深掘り", "検索・分類・並べ替えを組み合わせて探索。地域名を押すと、集計の前提も確認できます。", "配列のフィルター・並べ替え・選択", "8地域すべて架空・満足度は各地域100回答の仮定");
      const searchLabel = el("label", "lab-field", "地域を検索"), search = el("input");
      search.type = "search"; search.maxLength = 120; search.setAttribute("aria-label", "地域を検索"); searchLabel.append(search);
      const categoryLabel = el("label", "lab-field", "分類"), category = el("select");
      category.setAttribute("aria-label", "分類");
      ["全て", "都市", "郊外"].forEach(value => { const option = el("option", "", value); option.value = value; category.append(option); });
      categoryLabel.append(category);
      const sort = button("利用件数で並べ替え", () => { state.direction = state.direction === "asc" ? "desc" : "asc"; update(); });
      const direction = el("p", "lab-note"), count = live("result-count"), detail = live("record-detail");
      controls.append(searchLabel, categoryLabel, sort, direction, count, detail);
      const wrap = el("div", "lab-table-wrap"), table = el("table", "lab-table"), head = el("thead"), row = el("tr");
      table.append(el("caption", "", "架空の地域データ（満足度の分母：各100回答）"));
      ["地域", "分類", "利用件数", "満足度"].forEach(text => { const cell = el("th", "", text); cell.scope = "col"; row.append(cell); });
      head.append(row);
      const body = el("tbody"), empty = el("p", "lab-empty", "該当なし。検索条件を変更してください。");
      table.append(head, body); wrap.append(table, empty); visual.append(wrap);
      const normal = text => text.normalize("NFKC").trim().toLocaleLowerCase("ja-JP");
      let currentPage = 0;
      const pageStatus = live("table-page");
      const pageActions = el("div", "lab-table-pagination");
      const previousPage = button("前の行", () => { currentPage--; update(); });
      const nextPage = button("次の行", () => { currentPage++; update(); });
      pageActions.append(previousPage, pageStatus, nextPage); visual.append(pageActions);
      on(search, "input", () => { state.query = search.value.slice(0, 120); currentPage = 0; update(); });
      on(category, "change", () => { state.category = ["全て", "都市", "郊外"].includes(category.value) ? category.value : "全て"; currentPage = 0; update(); });
      function update() {
        const filtered = records.filter(record => normal(record[0]).includes(normal(state.query)) && (state.category === "全て" || record[1] === state.category))
          .sort((a, b) => state.direction === "asc" ? a[2] - b[2] : b[2] - a[2]);
        state.count = filtered.length;
        const perPage = canvasSize.height < 520 ? 1 : canvasSize.height < 620 ? 2 : 3;
        currentPage = Math.max(0, Math.min(currentPage, Math.ceil(filtered.length / perPage) - 1));
        const visible = filtered.slice(currentPage * perPage, (currentPage + 1) * perPage);
        if (!filtered.some(record => record[0] === state.selected)) state.selected = filtered[0]?.[0] ?? null;
        body.replaceChildren();
        for (const record of visible) {
          const tr = el("tr"), name = el("th"); name.scope = "row";
          const select = el("button", "lab-button", record[0]); select.type = "button";
          select.setAttribute("aria-pressed", String(record[0] === state.selected));
          // Rows are rebuilt on filter changes; delegated selection avoids retaining removed buttons.
          select.dataset.region = record[0]; name.append(select); tr.append(name);
          for (const value of [record[1], record[2].toLocaleString("ja-JP"), `${record[3]}%`]) tr.append(el("td", "", String(value)));
          body.append(tr);
        }
        empty.hidden = filtered.length !== 0; count.textContent = `${filtered.length}件 / 全8件${filtered.length ? "" : " · 該当なし"}`;
        direction.textContent = `並び順：利用件数の${state.direction === "asc" ? "少ない順" : "多い順"}`;
        const selected = filtered.find(record => record[0] === state.selected);
        detail.textContent = selected ? `${selected[0]}（${selected[1]}）：利用 ${selected[2].toLocaleString("ja-JP")}件。満足度 ${selected[3]}% = 満足 ${selected[3]}回答 / 架空の100回答。実測ではありません。` : "該当なし。表示できる地域はありません。";
        table.querySelectorAll("thead th")[2].setAttribute("aria-sort", state.direction === "asc" ? "ascending" : "descending");
        previousPage.disabled = currentPage === 0;
        nextPage.disabled = (currentPage + 1) * perPage >= filtered.length;
        pageStatus.textContent = filtered.length ? `${currentPage * perPage + 1}〜${currentPage * perPage + visible.length}行 / ${filtered.length}行` : "0行";
      }
      on(body, "click", event => {
        const target = event.target.closest?.("button[data-region]");
        if (!target || !body.contains(target)) return;
        state.selected = target.dataset.region; update();
        Array.from(body.querySelectorAll("button")).find(node => node.dataset.region === state.selected)?.focus();
      });
      update();
    },
    canvas() {
      state = { count: 60, selected: 1, running: false, points: [] };
      const { visual, controls } = card("点群を描き、1点を選んで調べる", "点を押すと最寄りの点を選択します。点の数を変えたり、明示的に動きを開始したりできます。", "Canvas描画・当たり判定・経過時間の計算", "決定的な種から作る架空の点・現実の観測ではない");
      const board = el("div", "lab-canvas-board"), canvas = el("canvas", "lab-canvas");
      canvas.setAttribute("role", "img"); canvas.setAttribute("aria-label", "架空の点群");
      canvas.textContent = "架空の点群。下の「点を選択」で各点の座標を確認できます。";
      board.append(canvas); visual.append(board);
      let context = null;
      try { context = canvas.getContext("2d"); } catch { /* Text controls remain available without Canvas support. */ }
      if (!context) {
        const error = el("p", "lab-error", "Canvas 2Dを利用できません。描画は停止しています。「点を選択」で点の座標は確認できます。");
        error.setAttribute("role", "alert"); visual.append(error);
      }
      let width = 800, height = 500;
      const detail = live("canvas-detail"), notice = el("p", "lab-note");
      const selectLabel = el("label", "lab-field", "点を選択"), select = el("select");
      select.setAttribute("aria-label", "点を選択"); selectLabel.append(select);
      range(controls, "点の数", 10, 120, 60, value => { state.count = value; seed(); resize(); });
      const move = button("動かす", () => { if (state.running) player.stop(); else player.start(); });
      controls.append(move, selectLabel, detail, notice);
      const player = animation(delta => {
        for (const point of state.points) {
          point.x += point.vx * delta; point.y += point.vy * delta;
          if (point.x < 0.04 || point.x > 0.96) { point.x = clamp(point.x, 0.04, 0.96); point.vx *= -1; }
          if (point.y < 0.06 || point.y > 0.94) { point.y = clamp(point.y, 0.06, 0.94); point.vy *= -1; }
        }
        draw(); return true;
      }, update);
      function seed() {
        let seedValue = 43129;
        const random = () => { seedValue = (Math.imul(seedValue, 1664525) + 1013904223) >>> 0; return seedValue / 4294967296; };
        state.points = Array.from({ length: state.count }, (_, index) => ({ id: index + 1, x: 0.06 + random() * 0.88, y: 0.08 + random() * 0.84, vx: (random() - 0.5) * 0.07, vy: (random() - 0.5) * 0.07 }));
        state.selected = Math.min(state.selected, state.count); select.replaceChildren();
        for (let index = 1; index <= state.count; index++) {
          const option = el("option", "", String(index)); option.value = String(index); select.append(option);
        }
        update();
      }
      function update() {
        move.textContent = state.running ? "止める" : "動かす";
        move.disabled = !context || Boolean(motion?.matches);
        notice.textContent = motion?.matches ? "動きを減らす設定のため停止中。点の選択と数の変更は利用できます。" : "初期状態は停止。非表示になった場合も停止します。";
        select.value = String(state.selected); draw(true);
      }
      function draw(announce = false) {
        const chosen = state.points[state.selected - 1];
        if (chosen && announce) detail.textContent = `点 ${chosen.id} / ${state.count}点：横 ${Math.round(chosen.x * 100)}%、縦 ${Math.round(chosen.y * 100)}%（架空の座標）。選択点はコーラル色と番号で表示。`;
        if (!context) return;
        const ctx = context;
        ctx.clearRect(0, 0, width, height); ctx.fillStyle = "#102a43"; ctx.fillRect(0, 0, width, height);
        ctx.strokeStyle = "#28465e"; ctx.lineWidth = 1;
        for (let index = 1; index < 10; index++) {
          ctx.beginPath(); ctx.moveTo(width * index / 10, 0); ctx.lineTo(width * index / 10, height); ctx.stroke();
          ctx.beginPath(); ctx.moveTo(0, height * index / 10); ctx.lineTo(width, height * index / 10); ctx.stroke();
        }
        for (const point of state.points) {
          if (point === chosen) continue;
          ctx.beginPath(); ctx.arc(point.x * width, point.y * height, 5, 0, Math.PI * 2); ctx.fillStyle = "#65dfd0"; ctx.fill();
        }
        if (chosen) {
          const x = chosen.x * width, y = chosen.y * height;
          ctx.beginPath(); ctx.arc(x, y, 10, 0, Math.PI * 2); ctx.fillStyle = "#ff9a83"; ctx.fill();
          ctx.strokeStyle = "#ffffff"; ctx.lineWidth = 2; ctx.stroke();
          ctx.fillStyle = "#ffffff"; ctx.font = "bold 18px sans-serif"; ctx.textBaseline = "bottom";
          ctx.textAlign = chosen.x > 0.7 ? "right" : "left"; ctx.fillText(`点 ${chosen.id}`, x + (chosen.x > 0.7 ? -14 : 14), Math.max(24, y - 12));
        }
      }
      function resize() {
        const box = board.getBoundingClientRect(), ratio = clamp(win.devicePixelRatio || 1, 1, 2);
        width = Math.max(1, box.width || 800); height = Math.max(1, box.height || 500);
        canvas.width = Math.round(width * ratio); canvas.height = Math.round(height * ratio);
        context?.setTransform(ratio, 0, 0, ratio, 0, 0); draw();
      }
      on(select, "change", () => { state.selected = Math.round(clamp(Number(select.value), 1, state.count)); update(); });
      on(canvas, "click", event => {
        const box = canvas.getBoundingClientRect(); if (!box.width || !box.height) return;
        const x = (event.clientX - box.left) / box.width, y = (event.clientY - box.top) / box.height;
        const distance = point => ((point.x - x) * box.width) ** 2 + ((point.y - y) * box.height) ** 2;
        const chosen = state.points.reduce((best, point) => distance(point) < distance(best) ? point : best);
        state.selected = chosen.id; update();
      });
      if (motion?.addEventListener) on(motion, "change", update);
      if (win.ResizeObserver) {
        const observer = new win.ResizeObserver(resize); observer.observe(board); sceneCleanups.push(() => observer.disconnect());
      } else on(win, "resize", resize);
      resizeCanvasScene = resize;
      sceneCleanups.push(() => { if (resizeCanvasScene === resize) resizeCanvasScene = null; });
      seed(); resize();
    }
  };
  const clearScene = () => { for (const cleanup of sceneCleanups.splice(0).reverse()) cleanup(); };
  const render = () => {
    clearScene(); main.replaceChildren();
    try { scenes[kind](); } catch (error) {
      clearScene();
      const message = el("p", "lab-error", "ラボを表示できませんでした。リセットして再試行してください。");
      message.setAttribute("role", "alert"); main.replaceChildren(message); throw error;
    }
  };
  const api = {
    dispose() {
      if (disposed) return;
      disposed = true; clearScene(); cleanups.splice(0).reverse().forEach(cleanup => cleanup());
      if (shell.parentNode === root) shell.remove();
      if (javascriptLabMounts.get(root) === api) {
        javascriptLabMounts.delete(root);
        if (!hadClass) root.classList.remove("js-lab-root");
      }
    },
    getState() {
      return JSON.parse(JSON.stringify({ kind, ...state }));
    }
  };
  javascriptLabMounts.set(root, api);
  on(explanationBack, "click", () => { explanationIndex = Math.max(0, explanationIndex - 1); showExplanation(); }, undefined, cleanups);
  on(explanationNext, "click", () => { explanationIndex = Math.min(explanationPages.length - 1, explanationIndex + 1); showExplanation(); }, undefined, cleanups);
  on(explanationToggle, "click", () => {
    const show = explanation.hidden;
    explanation.hidden = !show; main.hidden = show;
    explanationToggle.textContent = show ? "スライドに戻る" : "解説";
    explanationToggle.setAttribute("aria-expanded", String(show));
  }, undefined, cleanups);
  on(paneToggle, "click", () => {
    const controls = shell.dataset.pane !== "controls";
    shell.dataset.pane = controls ? "controls" : "visual";
    paneToggle.textContent = controls ? "図を表示" : "操作を表示";
    paneToggle.setAttribute("aria-pressed", String(controls));
    if (currentVisual && currentControls) {
      currentVisual.inert = controls; currentControls.inert = !controls;
    }
  }, undefined, cleanups);
  on(reset, "click", () => { if (!disposed) { render(); main.scrollTop = 0; } }, undefined, cleanups);
  try { render(); } catch (error) { api.dispose(); throw error; }
  return api;
}
