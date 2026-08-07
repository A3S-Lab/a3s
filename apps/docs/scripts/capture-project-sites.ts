import { execFile, spawn, type ChildProcess } from 'node:child_process';
import { constants } from 'node:fs';
import { access, mkdir, mkdtemp, readFile, readdir, rename, rm, stat, writeFile } from 'node:fs/promises';
import { homedir, platform, tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { featuredProjectSites } from '../components/home/project-sites';

const execute = promisify(execFile);
const root = process.cwd();
const outputDirectory = path.join(root, 'public', 'ecosystem-sites');

async function isExecutable(candidate: string) {
  try {
    await access(candidate, constants.X_OK);
    return true;
  } catch {
    return false;
  }
}

async function findOnPath(command: string) {
  try {
    const { stdout } = await execute('which', [command]);
    return stdout.trim() || undefined;
  } catch {
    return undefined;
  }
}

async function findAgentBrowserChrome() {
  const browserRoot = path.join(homedir(), '.agent-browser', 'browsers');

  try {
    const versions = (await readdir(browserRoot)).sort().reverse();
    for (const version of versions) {
      const candidate = platform() === 'darwin'
        ? path.join(browserRoot, version, 'Google Chrome for Testing.app', 'Contents', 'MacOS', 'Google Chrome for Testing')
        : path.join(browserRoot, version, 'chrome');
      if (await isExecutable(candidate)) return candidate;
    }
  } catch {
    return undefined;
  }

  return undefined;
}

async function findChrome() {
  const explicit = process.env.CHROME_BIN;
  if (explicit && await isExecutable(explicit)) return explicit;

  const platformCandidates = platform() === 'darwin'
    ? [
        '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
        '/Applications/Chromium.app/Contents/MacOS/Chromium',
      ]
    : [];

  for (const candidate of platformCandidates) {
    if (await isExecutable(candidate)) return candidate;
  }

  for (const command of ['google-chrome-stable', 'google-chrome', 'chromium', 'chromium-browser']) {
    const candidate = await findOnPath(command);
    if (candidate) return candidate;
  }

  return findAgentBrowserChrome();
}

async function ensureCjkFontSupport() {
  if (platform() !== 'linux') return;

  try {
    const { stdout } = await execute('fc-list', [':lang=zh-cn', 'family']);
    if (stdout.trim()) return;
  } catch {
    // Fall through to the actionable error below.
  }

  throw new Error(
    'No Chinese font is available for project previews. Install fonts-noto-cjk and refresh the font cache.',
  );
}

async function fileIsUsable(target: string) {
  try {
    return (await stat(target)).size > 20_000;
  } catch {
    return false;
  }
}

async function assertCaptureUrlIsHealthy(captureUrl: string) {
  const response = await fetch(captureUrl, {
    headers: { 'user-agent': 'a3s-site-preview-capture' },
    redirect: 'follow',
    signal: AbortSignal.timeout(15_000),
  });
  await response.body?.cancel();

  if (!response.ok) {
    throw new Error(`preview URL returned HTTP ${response.status}: ${captureUrl}`);
  }

  const contentType = response.headers.get('content-type') ?? '';
  if (!contentType.includes('text/html')) {
    throw new Error(`preview URL did not return HTML (${contentType || 'unknown content type'}): ${captureUrl}`);
  }
}

function delay(milliseconds: number) {
  return new Promise<void>((resolve) => setTimeout(resolve, milliseconds));
}

async function withTimeout<T>(operation: Promise<T>, milliseconds: number, message: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => reject(new Error(message)), milliseconds);
  });

  try {
    return await Promise.race([operation, timeout]);
  } finally {
    if (timer) clearTimeout(timer);
  }
}

async function waitForDebugPort(profile: string, chromeProcess: ChildProcess) {
  const portFile = path.join(profile, 'DevToolsActivePort');
  const deadline = Date.now() + 15_000;

  while (Date.now() < deadline) {
    if (chromeProcess.exitCode !== null) {
      throw new Error(`Chrome exited before its debugging endpoint was ready (${chromeProcess.exitCode})`);
    }

    try {
      const [rawPort] = (await readFile(portFile, 'utf8')).split('\n');
      const port = Number(rawPort);
      if (Number.isInteger(port) && port > 0) return port;
    } catch {
      // Chrome writes DevToolsActivePort after the browser process is ready.
    }

    await delay(50);
  }

  throw new Error('Chrome debugging endpoint did not become ready within 15 seconds');
}

async function stopChrome(chromeProcess: ChildProcess) {
  if (chromeProcess.exitCode !== null) return;

  chromeProcess.kill('SIGTERM');
  await Promise.race([
    new Promise<void>((resolve) => chromeProcess.once('exit', () => resolve())),
    delay(3_000),
  ]);

  if (chromeProcess.exitCode === null) {
    chromeProcess.kill('SIGKILL');
    await Promise.race([
      new Promise<void>((resolve) => chromeProcess.once('exit', () => resolve())),
      delay(2_000),
    ]);
  }
}

async function waitForPageTarget(port: number, chromeProcess: ChildProcess) {
  const deadline = Date.now() + 5_000;

  while (Date.now() < deadline) {
    if (chromeProcess.exitCode !== null) {
      throw new Error(`Chrome exited before exposing a page target (${chromeProcess.exitCode})`);
    }

    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`, {
        signal: AbortSignal.timeout(2_000),
      });
      if (response.ok) {
        const targets = await response.json() as Array<{ type?: string; webSocketDebuggerUrl?: string }>;
        const pageTarget = targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
        if (pageTarget?.webSocketDebuggerUrl) return pageTarget.webSocketDebuggerUrl;
      }
    } catch {
      // The endpoint can become reachable one tick before the initial page target.
    }

    await delay(100);
  }

  throw new Error('Chrome exposed no page debugging target within 5 seconds');
}

interface CdpMessage {
  id?: number;
  method?: string;
  params?: unknown;
  result?: Record<string, unknown>;
  error?: { message?: string };
}

async function openWebSocket(url: string) {
  const socket = new WebSocket(url);
  await withTimeout(
    new Promise<void>((resolve, reject) => {
      socket.addEventListener('open', () => resolve(), { once: true });
      socket.addEventListener('error', () => reject(new Error('Chrome debugging WebSocket failed to open')), { once: true });
    }),
    10_000,
    'Chrome debugging WebSocket did not open within 10 seconds',
  );
  return socket;
}

function createCdpClient(socket: WebSocket) {
  let nextId = 0;
  const pending = new Map<number, { resolve: (result: Record<string, unknown>) => void; reject: (error: Error) => void }>();
  const eventWaiters = new Map<string, Set<(params: unknown) => void>>();

  socket.addEventListener('message', (event) => {
    if (typeof event.data !== 'string') return;
    const message = JSON.parse(event.data) as CdpMessage;

    if (message.id !== undefined) {
      const request = pending.get(message.id);
      if (!request) return;
      pending.delete(message.id);
      if (message.error) request.reject(new Error(message.error.message ?? 'Chrome debugging command failed'));
      else request.resolve(message.result ?? {});
      return;
    }

    if (!message.method) return;
    const waiters = eventWaiters.get(message.method);
    if (!waiters) return;
    eventWaiters.delete(message.method);
    for (const resolve of waiters) resolve(message.params);
  });

  socket.addEventListener('close', () => {
    for (const request of pending.values()) request.reject(new Error('Chrome debugging WebSocket closed'));
    pending.clear();
  });

  function send(method: string, params: Record<string, unknown> = {}) {
    const id = ++nextId;
    const operation = new Promise<Record<string, unknown>>((resolve, reject) => {
      pending.set(id, { resolve, reject });
      socket.send(JSON.stringify({ id, method, params }));
    });
    return withTimeout(operation, 30_000, `Chrome debugging command timed out: ${method}`);
  }

  function waitForEvent(method: string) {
    const operation = new Promise<unknown>((resolve) => {
      const waiters = eventWaiters.get(method) ?? new Set();
      waiters.add(resolve);
      eventWaiters.set(method, waiters);
    });
    return withTimeout(operation, 30_000, `Chrome debugging event timed out: ${method}`);
  }

  return { send, waitForEvent };
}

async function captureWithChrome(
  captureUrl: string,
  output: string,
  profile: string,
  settleMs: number,
) {
  await mkdir(profile, { recursive: true });
  const chromeProcess = spawn(chromeExecutable, [
    '--headless=new',
    '--disable-background-networking',
    '--disable-dev-shm-usage',
    '--disable-gpu',
    '--hide-scrollbars',
    '--lang=zh-CN',
    '--no-default-browser-check',
    '--no-first-run',
    '--no-sandbox',
    '--remote-allow-origins=*',
    '--remote-debugging-port=0',
    '--window-size=1280,800',
    `--user-data-dir=${profile}`,
    'about:blank',
  ], { stdio: 'ignore' });
  let socket: WebSocket | undefined;

  try {
    const port = await waitForDebugPort(profile, chromeProcess);
    const pageTargetUrl = await waitForPageTarget(port, chromeProcess);
    socket = await openWebSocket(pageTargetUrl);
    const client = createCdpClient(socket);
    await client.send('Page.enable');
    await client.send('Runtime.enable');
    await client.send('Emulation.setDeviceMetricsOverride', {
      deviceScaleFactor: 1,
      height: 800,
      mobile: false,
      width: 1280,
    });
    const loaded = client.waitForEvent('Page.loadEventFired');
    const navigation = await client.send('Page.navigate', { url: captureUrl });
    if (typeof navigation.errorText === 'string' && navigation.errorText) {
      throw new Error(`Chrome could not navigate to ${captureUrl}: ${navigation.errorText}`);
    }
    await loaded;
    await client.send('Runtime.evaluate', {
      awaitPromise: true,
      expression: `(async () => {
        if (document.fonts?.ready) await document.fonts.ready;
        const images = [...document.images].filter((image) => image.loading !== 'lazy' || image.getBoundingClientRect().top < innerHeight * 1.5);
        await Promise.all(images.map((image) => image.complete ? undefined : new Promise((resolve) => {
          image.addEventListener('load', resolve, { once: true });
          image.addEventListener('error', resolve, { once: true });
        })));
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        await new Promise((resolve) => setTimeout(resolve, ${settleMs}));

        const captureStyle = document.createElement('style');
        captureStyle.dataset.a3sCapture = 'frozen';
        captureStyle.textContent = \`
          *, *::before, *::after {
            animation-play-state: paused !important;
            caret-color: transparent !important;
            scroll-behavior: auto !important;
            transition: none !important;
          }
        \`;
        document.head.append(captureStyle);

        for (const animation of document.getAnimations()) animation.pause();
        for (const video of document.querySelectorAll('video')) video.pause();
        for (const svg of document.querySelectorAll('svg')) {
          if ('pauseAnimations' in svg && typeof svg.pauseAnimations === 'function') svg.pauseAnimations();
        }

        for (const image of document.images) {
          if (!/\\.gif(?:$|[?#])/i.test(image.currentSrc) || !image.naturalWidth || !image.naturalHeight) continue;
          const canvas = document.createElement('canvas');
          canvas.width = image.naturalWidth;
          canvas.height = image.naturalHeight;
          const context = canvas.getContext('2d');
          if (!context) continue;
          try {
            context.drawImage(image, 0, 0);
            image.src = canvas.toDataURL('image/png');
          } catch {
            // Cross-origin images can keep animating; the page is otherwise ready to capture.
          }
        }

        document.documentElement.dataset.a3sCaptureReady = 'true';
        await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
        return document.title;
      })()`,
      returnByValue: true,
    });
    const screenshot = await client.send('Page.captureScreenshot', {
      captureBeyondViewport: false,
      format: 'png',
      fromSurface: true,
    });
    if (typeof screenshot.data !== 'string') throw new Error('Chrome returned no screenshot data');
    await writeFile(output, Buffer.from(screenshot.data, 'base64'));
  } finally {
    socket?.close();
    await stopChrome(chromeProcess);
  }
}

const requestedSite = process.argv.find((argument) => argument.startsWith('--site='))?.split('=')[1];
const sites = requestedSite
  ? featuredProjectSites.filter((site) => site.id === requestedSite)
  : [...featuredProjectSites];

if (requestedSite && sites.length === 0) {
  throw new Error(`Unknown project site: ${requestedSite}`);
}

const chrome = await findChrome();
if (!chrome) {
  throw new Error('Chrome was not found. Set CHROME_BIN or install Chrome/Chromium before capturing project sites.');
}
const chromeExecutable: string = chrome;

await ensureCjkFontSupport();

await mkdir(outputDirectory, { recursive: true });
const temporaryDirectory = await mkdtemp(path.join(tmpdir(), 'a3s-project-sites-'));
const failures: string[] = [];

async function capture(site: (typeof featuredProjectSites)[number]) {
  const output = path.join(outputDirectory, path.basename(site.screenshot));
  const temporaryOutput = path.join(temporaryDirectory, path.basename(site.screenshot));
  const profile = path.join(temporaryDirectory, `${site.id}-profile`);
  const captureUrl = site.id === 'form'
    ? process.env.A3S_FORM_PREVIEW_URL ?? site.captureUrl
    : site.captureUrl;

  try {
    await assertCaptureUrlIsHealthy(captureUrl);
    await captureWithChrome(captureUrl, temporaryOutput, profile, site.settleMs);

    if (!await fileIsUsable(temporaryOutput)) {
      throw new Error('Chrome produced no usable screenshot');
    }

    await rename(temporaryOutput, output);
    console.log(`Captured ${site.id}: ${captureUrl}`);
  } catch (error) {
    if (await fileIsUsable(temporaryOutput)) {
      await rename(temporaryOutput, output);
      console.warn(`Captured ${site.id}: ${captureUrl} (Chrome reported an error after writing the image)`);
      return;
    }

    if (await fileIsUsable(output)) {
      console.warn(`Keeping the committed ${site.id} screenshot: ${String(error)}`);
      return;
    }

    failures.push(`${site.id}: ${String(error)}`);
  }
}

try {
  let nextIndex = 0;
  async function worker() {
    while (nextIndex < sites.length) {
      const site = sites[nextIndex];
      nextIndex += 1;
      await capture(site);
    }
  }

  await Promise.all([worker(), worker()]);
} finally {
  await rm(temporaryDirectory, { force: true, recursive: true });
}

if (failures.length > 0) {
  throw new Error(`Project site capture failed:\n${failures.join('\n')}`);
}
