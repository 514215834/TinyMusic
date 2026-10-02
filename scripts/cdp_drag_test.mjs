/* CDP 拖拽验证：模拟指针拖拽侧边栏歌单行，验证排序生效且普通点击仍导航 */
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

  await send("Runtime.enable");
  await send("Page.enable");

  const names = () => evaluate(`[...document.querySelectorAll('.pl-list .pl-item .pl-name')].map(e => e.textContent)`);
  const rects = () => evaluate(`[...document.querySelectorAll('.pl-list .pl-item')].map(e => { const r = e.getBoundingClientRect(); return { x: r.x + r.width / 2, y: r.y + r.height / 2 }; })`);

  const namesBefore = names();
  console.log("初始顺序:", JSON.stringify(await namesBefore));

  // 1. 拖拽第 2 行（拖拽测试A）到第 3 行位置
  const rs = await rects();
  const from = rs[1], to = rs[2];
  if (!from || !to) throw new Error("playlists rows missing: " + JSON.stringify(rs));
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x: Math.round(from.x), y: Math.round(from.y), button: "left", buttons: 1, clickCount: 1, pointerType: "mouse" });
  await sleep(60);
  const STEPS = 8;
  for (let i = 1; i <= STEPS; i++) {
    await send("Input.dispatchMouseEvent", {
      type: "mouseMoved",
      x: Math.round(from.x + ((to.x - from.x) * i) / STEPS),
      y: Math.round(from.y + ((to.y - from.y) * i) / STEPS),
      buttons: 1, pointerType: "mouse",
    });
    await sleep(40);
  }
  // 松开前先看落点指示是否激活
  const dragState = await evaluate(`(() => { const d = document.querySelector('.pl-item.pl-dragging .pl-name'); const o = document.querySelector('.pl-item.pl-over .pl-name'); return { dragging: d?.textContent ?? null, over: o?.textContent ?? null }; })()`);
  console.log("拖拽态:", JSON.stringify(dragState));
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: Math.round(to.x), y: Math.round(to.y), button: "left", buttons: 0, clickCount: 1, pointerType: "mouse" });
  await sleep(300);
  const afterDrag = await names();
  console.log("拖拽后顺序:", JSON.stringify(afterDrag));

  // 2. 普通点击第 1 行应正常导航（不被拖拽逻辑劫持）
  const rs2 = await rects();
  await send("Input.dispatchMouseEvent", { type: "mousePressed", x: Math.round(rs2[0].x), y: Math.round(rs2[0].y), button: "left", buttons: 1, clickCount: 1, pointerType: "mouse" });
  await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: Math.round(rs2[0].x), y: Math.round(rs2[0].y), button: "left", buttons: 0, clickCount: 1, pointerType: "mouse" });
  await sleep(400);
  const route = await evaluate(`location.pathname`);
  console.log("点击后路由:", route);

  // 状态无关断言：第 2、3 行应互换，且拖拽指示与导航正常
  const initial = await namesBefore;
  const ok =
    dragState.dragging === initial[1] &&
    afterDrag[1] === initial[2] && afterDrag[2] === initial[1] &&
    route.startsWith("/playlists/");
  console.log(ok ? "PASS" : "FAIL");
  process.exit(ok ? 0 : 1);
}

main().catch((e) => { console.error("ERROR:", e.message); process.exit(2); });
