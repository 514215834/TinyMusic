/* 压测：多轮拖拽与点击交替 + 连续拖拽，断言拖拽后每次点击导航都即时响应 */
const CDP_HTTP = "http://127.0.0.1:9222/json";

async function main() {
  const targets = await (await fetch(CDP_HTTP)).json();
  const page = targets.find((t) => t.url.startsWith("http://localhost:1420"));
  if (!page) throw new Error("TinyMusic page target not found");
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });

  let msgId = 0;
  const pending = new Map();
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) {
      const p = pending.get(m.id);
      pending.delete(m.id);
      if (m.error) p.rej(new Error(m.error.message));
      else p.res(m.result);
    }
  };
  const send = (method, params = {}) =>
    new Promise((res, rej) => {
      const id = ++msgId;
      pending.set(id, { res, rej });
      ws.send(JSON.stringify({ id, method, params }));
    });
  const evaluate = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const rowRects = () =>
    evaluate(`[...document.querySelectorAll('.pl-list .pl-item')].map(e => { const r = e.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })`);
  const navRect = (kw) =>
    evaluate(`(() => { const el = [...document.querySelectorAll('.nav-item')].find(a => a.getAttribute('href').includes('${kw}')); const r = el.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })()`);
  const rawClick = async (pt) => {
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x: Math.round(pt.x), y: Math.round(pt.y), button: "left", buttons: 1, clickCount: 1, pointerType: "mouse" });
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: Math.round(pt.x), y: Math.round(pt.y), button: "left", buttons: 0, clickCount: 1, pointerType: "mouse" });
  };
  const dragRow = async (fromIdx, toIdx) => {
    const rs = await rowRects();
    const from = rs[fromIdx], to = rs[toIdx];
    await send("Input.dispatchMouseEvent", { type: "mousePressed", x: Math.round(from.x), y: Math.round(from.y), button: "left", buttons: 1, clickCount: 1, pointerType: "mouse" });
    await sleep(50);
    for (let i = 1; i <= 6; i++) {
      await send("Input.dispatchMouseEvent", {
        type: "mouseMoved",
        x: Math.round(from.x + ((to.x - from.x) * i) / 6),
        y: Math.round(from.y + ((to.y - from.y) * i) / 6),
        buttons: 1, pointerType: "mouse",
      });
      await sleep(35);
    }
    await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: Math.round(to.x), y: Math.round(to.y), button: "left", buttons: 0, clickCount: 1, pointerType: "mouse" });
    await sleep(250);
  };

  await send("Runtime.enable");
  await send("Page.enable");
  await evaluate(`history.pushState({}, '', '/library'); window.dispatchEvent(new PopStateEvent('popstate'));`);
  await sleep(300);

  let failures = [];
  const expectRoute = async (want, label) => {
    await sleep(350);
    const route = await evaluate(`location.pathname`);
    console.log(`${label}: ${route} ${route === want ? "OK" : "FAIL（期望 " + want + "）"}`);
    if (route !== want) failures.push(label);
  };

  // 第 1 轮：拖拽 → 立即点击设置
  await dragRow(1, 2);
  await rawClick(await navRect("settings"));
  await expectRoute("/settings", "轮1 拖后点击设置");
  await rawClick(await navRect("library"));
  await expectRoute("/library", "轮1 点击返回曲库");

  // 第 2 轮：反向拖拽 → 立即点击设置
  await dragRow(2, 1);
  await rawClick(await navRect("settings"));
  await expectRoute("/settings", "轮2 拖后点击设置");
  await rawClick(await navRect("library"));
  await expectRoute("/library", "轮2 点击返回曲库");

  // 第 3 轮：连续两次拖拽（中间无点击）→ 立即点击设置
  await dragRow(1, 2);
  await dragRow(2, 1);
  await rawClick(await navRect("settings"));
  await expectRoute("/settings", "轮3 连续拖拽后点击设置");
  await rawClick(await navRect("library"));
  await expectRoute("/library", "轮3 点击返回曲库");

  // 第 4 轮：连续四次拖拽 → 点击
  for (let i = 0; i < 4; i++) await dragRow(1, 2);
  await rawClick(await navRect("settings"));
  await expectRoute("/settings", "轮4 连续四次拖拽后点击设置");

  if (failures.length) {
    console.log("FAIL：", failures.join("; "));
    process.exit(1);
  }
  console.log("PASS（全部点击即时响应，拖拽排序正常）");
  process.exit(0);
}

main().catch((e) => { console.error("ERROR:", e.message); process.exit(2); });
