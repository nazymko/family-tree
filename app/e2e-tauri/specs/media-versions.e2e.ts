/**
 * Versions of one page, on the REAL app: a large scan and a small older copy of the same (invented) register page are
 * grouped from the viewer. The gallery then shows ONE tile with a «2 versions» badge, and the viewer opens the scan
 * with a switcher to the copy, whose own transcription is shown when it is on screen.
 *
 * Synthetic data only: canvas-drawn pages with an invented entry. Screenshots in English and Ukrainian.
 */
import fs from "node:fs";
import path from "node:path";
import { expect } from "@wdio/globals";
import {
  $,
  $$,
  browser,
  click,
  drop,
  goTo,
  importSynthetic,
  openTreeFromReport,
  ready,
  shot,
  slash,
  work,
} from "./helpers";

/** One invented register page drawn at `w` by `h`; the copy is paler, as an old photocopy is. */
async function page(
  file: string,
  w: number,
  h: number,
  faded: boolean,
): Promise<string> {
  const b64 = await browser.execute(
    (w: number, h: number, faded: boolean) => {
      const c = document.createElement("canvas");
      c.width = w;
      c.height = h;
      const g = c.getContext("2d")!;
      const s = w / 1200;
      g.fillStyle = faded ? "#e9e2cf" : "#f3ead2";
      g.fillRect(0, 0, w, h);
      g.strokeStyle = faded ? "#b9ae96" : "#8c7a5a";
      g.lineWidth = 2 * s;
      for (let y = 140; y < 900; y += 90) {
        g.beginPath();
        g.moveTo(60 * s, y * s);
        g.lineTo(1140 * s, y * s);
        g.stroke();
      }
      for (const x of [60, 220, 640, 1140]) {
        g.beginPath();
        g.moveTo(x * s, 60 * s);
        g.lineTo(x * s, 860 * s);
        g.stroke();
      }
      g.fillStyle = faded ? "#6d6455" : "#2b2418";
      g.font = `italic ${38 * s}px serif`;
      g.fillText("Parish register of Zelenyi Hai, 1901", 260 * s, 110 * s);
      g.font = `italic ${32 * s}px serif`;
      g.fillText("12", 110 * s, 200 * s);
      g.fillText("Marta Hrytsenko, born 3 May", 250 * s, 200 * s);
      g.fillText("father Ostap, mother Paraska", 250 * s, 290 * s);
      g.fillText("13", 110 * s, 380 * s);
      g.fillText("Ivan Koval, born 9 May", 250 * s, 380 * s);
      return c.toDataURL("image/jpeg", faded ? 0.6 : 0.92).split(",")[1];
    },
    w,
    h,
    faded,
  );
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, Buffer.from(b64, "base64"));
  return slash(file);
}

/** Waits until the picture in the viewer has loaded, so a screenshot shows it. */
async function pictureLoaded() {
  await browser.waitUntil(
    () =>
      browser.execute(() => {
        const i = document.querySelector<HTMLImageElement>(
          '[role="dialog"] .frame img',
        );
        return !!i && i.complete && i.naturalWidth > 0;
      }),
    { timeout: 30000, timeoutMsg: "the picture should load" },
  );
}

async function chooseLanguage(code: string, heading: string) {
  const nav = await $(
    '//nav//a[normalize-space(.)="Settings" or normalize-space(.)="Налаштування"]',
  );
  await nav.click();
  const select = await $(
    `//select[@id=//label[starts-with(normalize-space(.),"Language") or starts-with(normalize-space(.),"Мова")]/@for]`,
  );
  await select.waitForDisplayed();
  await select.selectByAttribute("value", code);
  await $(`h1=${heading}`).waitForDisplayed({ timeout: 20000 });
}

const tiles = () => $$('ul[aria-label="Photos and documents"] li');

describe("Versions of one page", () => {
  before(async () => {
    await ready();
  });

  it("groups a scan and an older copy into one tile with a version switcher", async () => {
    await importSynthetic("Media versions tree");
    await openTreeFromReport();
    await goTo("Media");
    await $("h1=Media").waitForDisplayed();
    const dir = path.join(work(), "media-versions");
    const scan = await page(
      path.join(dir, "register-1901-p12-scan.jpg"),
      2400,
      1800,
      false,
    );
    const copy = await page(
      path.join(dir, "register-1901-p12-copy.jpg"),
      800,
      600,
      true,
    );
    await $("button=Add files…").waitForDisplayed();
    const before = (await tiles()).length;
    await drop([scan, copy]);
    await browser.waitUntil(async () => (await tiles()).length === before + 2, {
      timeoutMsg: "both pages should appear",
    });

    // The old copy carries the transcription someone typed long ago.
    await (
      await $('button[aria-label="View register-1901-p12-copy.jpg"]')
    ).click();
    await $('[role="dialog"] .frame').waitForDisplayed();
    await click("Edit text");
    await click("Add text without a place");
    await browser.keys(
      "No. 12. Marta Hrytsenko, born 3 May 1901; father Ostap, mother Paraska.",
    );
    await browser.keys("Tab");
    await click("Edit text");

    // Grouped from the viewer: the scan has the most pixels, so it becomes the primary.
    await click("Group with another version…");
    const pick = await $(
      'button[aria-label="Group with register-1901-p12-scan.jpg"]',
    );
    await pick.waitForClickable({ timeout: 20000 });
    await pick.click();
    const switcher = await $('[data-testid="version-switcher"]');
    await switcher.waitForDisplayed({ timeout: 20000 });
    const labels = await switcher.$$("button").map((b) => b.getText());
    expect(labels).toEqual(["primary 2400×1800", "copy 800×600"]);
    expect(await $('[role="dialog"] .frame img').getAttribute("alt")).toBe(
      "register-1901-p12-scan.jpg",
    );
    await $('[data-testid="version-text-hint"]').waitForDisplayed();
    await pictureLoaded();
    await shot("media-versions-viewer-primary-en.png");
    await click("copy 800×600");
    await browser.waitUntil(
      async () =>
        (await $('[role="dialog"] .frame img').getAttribute("alt")) ===
        "register-1901-p12-copy.jpg",
    );
    await pictureLoaded();
    await shot("media-versions-viewer-copy-en.png");
    await browser.keys("Escape");
    await $('[role="dialog"]').waitForExist({ reverse: true, timeout: 10000 });

    // One tile for the page, with a badge.
    await browser.waitUntil(async () => (await tiles()).length === before + 1, {
      timeoutMsg: "the page should show as one tile",
    });
    const badge = await $('[data-testid="versions-badge"]');
    expect(await badge.getText()).toBe("2 versions");
    await shot("media-versions-gallery-en.png");
    await $('button[aria-label="View register-1901-p12-scan.jpg"]').moveTo();
    await $('[data-testid="media-tip"]').waitForDisplayed({ timeout: 5000 });
    await shot("media-versions-gallery-tip-en.png");
  });

  it("shows the same in Ukrainian", async () => {
    await chooseLanguage("uk", "Налаштування");
    await $('//nav//a[normalize-space(.)="Медіа"]').click();
    const badge = await $('[data-testid="versions-badge"]');
    await badge.waitForDisplayed({ timeout: 20000 });
    expect(await badge.getText()).toBe("2 версії");
    await shot("media-versions-gallery-uk.png");
    await $('ul[aria-label="Фото та документи"] button').click();
    const switcher = await $('[data-testid="version-switcher"]');
    await switcher.waitForDisplayed({ timeout: 20000 });
    const labels = await switcher.$$("button").map((b) => b.getText());
    expect(labels).toEqual(["основна 2400×1800", "копія 800×600"]);
    await pictureLoaded();
    await shot("media-versions-viewer-uk.png");
    await browser.keys("Escape");
    await chooseLanguage("en", "Settings");
  });
});
