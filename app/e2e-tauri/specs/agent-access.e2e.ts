/**
 * Agent access on the real app: turn it on, make a connection, and let a scripted client (plain `fetch` from Node, the
 * way an MCP client would) work while the app is open. The view must refresh with no action, the card must carry the
 * badge, the log must name the agent, reviewing a batch must clear the badges, and undoing it must restore the tree.
 * Screenshots go to $GT_E2E_SHOTS in English, Ukrainian and the dark theme.
 *
 * Needs a debug build and GT_E2E=1 (wdio.conf.ts sets it). With the e2e config folder the key hashes go to a file in it,
 * never to the keychain of the person running the test.
 */
import { expect } from '@wdio/globals';
import { $, $$, browser, click, clearFocus, goTo, importSynthetic, meta, openTreeFromReport, ready, search, shot, text } from './helpers';

const TREE = 'Agent tree';
const PORT = 48765;
const AGENT = 'Transcript reader';
const TITLE = 'Recording 2 summary';
const CHILD = 'Zenobia';
const HITS = '[role="listbox"][aria-label="Search results"] [role="option"]';
const BADGE = 'span[title^="An agent changed this person"]';

/** A bare-bones MCP client: JSON-RPC over HTTP with the key as a bearer token. */
class Client {
  constructor(private key: string) {}

  // The answers are read loosely on purpose: this is a throwaway client for one test.
  /* eslint-disable @typescript-eslint/no-explicit-any */
  async rpc(method: string, params: unknown, key = this.key): Promise<{ status: number; body: any }> {
    const res = await fetch(`http://127.0.0.1:${PORT}/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', Accept: 'application/json, text/event-stream', Authorization: `Bearer ${key}` },
      body: JSON.stringify({ jsonrpc: '2.0', id: 1, method, params }),
    });
    return { status: res.status, body: await res.json().catch(() => null) };
  }

  /** Calls a tool; returns its parsed answer, or `{ error }` with the message when the tool refused. */
  async tool(name: string, args: Record<string, unknown>): Promise<any> {
    const { status, body } = await this.rpc('tools/call', { name, arguments: args });
    if (status !== 200) throw new Error(`${name}: HTTP ${status}`);
    const result = body.result;
    const first = result.content[0].text as string;
    return result.isError ? { error: first } : JSON.parse(first);
  }

  /* eslint-enable @typescript-eslint/no-explicit-any */

  async people(): Promise<number> {
    return (await this.tool('tree_summary', {})).counts.people;
  }

  async logTotal(): Promise<number> {
    return (await this.tool('get_log', { limit: 1 })).total;
  }

  /** The work on the home person, as an agent that follows the rules would do it. Safe to run twice. */
  async work(homeId: string, livingId: string) {
    const home = meta().home;
    const { batch } = await this.tool('begin_batch', { title: TITLE, source_file: 'transcript_summary.md' });
    const child = await this.tool('add_person', {
      batch, given: CHILD, surnames: [home.surname], sex: 'female', birth_date: '1932', source_file: 'transcript_summary.md', number: '4',
      relation: { to: homeId, as: 'child' },
    });
    expect(child.error).toBeUndefined();
    const note = await this.tool('add_note', { batch, target: { kind: 'person', id: homeId }, text: 'The recording gives another patronymic than the tree.' });
    expect(note.error).toBeUndefined();
    const marker = await this.tool('add_sibling_marker', { batch, sibling_of: homeId });
    expect(marker.error).toBeUndefined();
    // Death of someone the app counts as living: refused, in the agent's own words.
    const refused = await this.tool('add_event', { batch, person: livingId, kind: 'death', date: '2019' });
    expect(refused.error).toContain('alive or dead');
    // A date that reads two ways: refused.
    const ambiguous = await this.tool('add_event', { batch, person: homeId, kind: 'residence', date: '03/04/1927' });
    expect(ambiguous.error).toContain('two ways');
    await this.tool('end_batch', { batch, report: 'Added one daughter and the marker. Not changed: the death of a living person.' });
  }
}

/** The first search hit with this surname that the app counts as living. */
async function livingPerson(client: Client): Promise<string> {
  const surname = meta().uniqueLivingSurnames[0];
  const found = await client.tool('search_people', { query: surname, limit: 50 });
  const row = found.items.find((p: { living: boolean }) => p.living === true);
  if (!row) throw new Error(`no living person among the hits for ${surname}`);
  return row.id;
}

async function homeId(client: Client): Promise<string> {
  const found = await client.tool('search_people', { query: meta().home.name, limit: 50 });
  const row = found.items.find((p: { name: string }) => p.name === meta().home.name) ?? found.items[0];
  return row.id;
}

/** The select that belongs to a label, whatever language the label is in. */
async function choose(label: string, value: string) {
  const select = await $(`//select[@id=//label[starts-with(normalize-space(.),"${label}")]/@for]`);
  await select.waitForDisplayed();
  await select.selectByAttribute('value', value);
}

// By its link only: the nav's own label is translated too, and this spec switches language.
const nav = (name: string) => $(`//nav//a[normalize-space(.)="${name}"]`);

async function open(name: string) {
  const a = await nav(name);
  await a.waitForClickable();
  await a.click();
}

async function focusHome() {
  await search(meta().home.name);
  await $(HITS).waitForDisplayed();
  await browser.keys('Enter');
  await $(`//h1[contains(.,"${meta().home.given}")]`).waitForDisplayed();
}

const badges = async () => (await $$(BADGE)).length;

/** The agent panel on the person page: scrolled to, and how its count reads. */
async function panelCount(): Promise<string> {
  const count = await $('[data-testid="agent-count"]');
  await count.waitForExist({ timeout: 20000 });
  await count.scrollIntoView({ block: 'center' });
  return (await $('[data-testid="agent-count"] [aria-hidden]').getText()).trim();
}

const panelOpen = () => browser.execute(() => document.querySelector('[data-testid="agent-count"]')!.closest('details')!.open);

describe('Agent access end to end', () => {
  let client: Client;
  let home = '';
  let living = '';
  let peopleBefore = 0;

  before(async () => {
    await ready();
  });

  it('turns access on and shows the key once', async () => {
    await importSynthetic(TREE);
    await openTreeFromReport();
    await goTo('Settings');

    const port = await $('aria/Port');
    await port.waitForDisplayed();
    await port.setValue(String(PORT));
    await browser.keys('Tab');
    await (await $('aria/Let agents on this computer connect')).click();
    await $(`//p[@role="status"][contains(.,"Listening on 127.0.0.1:${PORT}")]`).waitForExist({ timeout: 20000 });

    await (await $('aria/Name for a new connection')).setValue(AGENT);
    await click('Create connection');
    const key = await $('[data-testid="agent-key"]');
    await key.waitForDisplayed();
    const secret = (await key.getText()).trim();
    expect(secret.length).toBeGreaterThan(20);
    const snippet = await $('[data-testid="agent-snippet"]').getText();
    expect(snippet).toContain(`claude mcp add --transport http gene-tree http://127.0.0.1:${PORT}/mcp`);
    expect(snippet).toContain(secret);
    await shot('agent-key-en.png');
    await click('Done');
    // The key is gone from the page for good.
    expect(await text()).not.toContain(secret);
    await shot('agent-settings-en.png');

    client = new Client(secret);
    // A wrong key gets nothing, and the right one reads the tree.
    expect((await client.rpc('tools/list', {}, 'not-the-key')).status).toBe(401);
    peopleBefore = await client.people();
    expect(peopleBefore).toBeGreaterThan(0);
    home = await homeId(client);
    living = await livingPerson(client);
  });

  it('refreshes the view and marks the card while the agent writes', async () => {
    await goTo('Family');
    await focusHome();
    expect(await text()).not.toContain(CHILD);
    expect(await badges()).toBe(0);

    await client.work(home, living);

    // No click, no reload: the new daughter appears and the cards carry the badge.
    await browser.waitUntil(async () => (await text()).includes(CHILD), { timeout: 20000, timeoutMsg: 'the family view did not refresh by itself' });
    await browser.waitUntil(async () => (await badges()) > 0, { timeout: 20000, timeoutMsg: 'no badge on the changed cards' });
    expect(await client.people()).toBe(peopleBefore + 1);
    // The living person is still living: nothing an agent writes changes that.
    const row = (await client.tool('search_people', { query: meta().uniqueLivingSurnames[0], limit: 50 })).items.find((p: { id: string }) => p.id === living);
    expect(row.living).toBe(true);
    expect(row.deathYear).toBeNull();
    await browser.pause(600);
    await shot('agent-family-en.png');
  });

  it('names the agent in the change log', async () => {
    await open('Change log');
    await $('h1=Change log').waitForDisplayed();
    await $(`h2*=${TITLE}`).waitForDisplayed({ timeout: 20000 });
    const body = await text();
    expect(body).toContain(`By ${AGENT}`);
    expect(body).toContain('From transcript_summary.md');
    expect(body).toContain('Agent report');
    expect(body).toContain('Not changed: the death of a living person.');
    await browser.pause(400);
    await shot('agent-log-en.png');
  });

  it('shows the same in Ukrainian and in the dark theme', async () => {
    await open('Settings');
    await choose('Language', 'uk');
    await $('h1=Налаштування').waitForDisplayed({ timeout: 20000 });
    await shot('agent-settings-uk.png');

    await open('Журнал змін');
    await $('h1=Журнал змін').waitForDisplayed();
    await $(`h2*=${TITLE}`).waitForDisplayed();
    expect(await text()).toContain(`Автор: ${AGENT}`);
    await browser.pause(400);
    await shot('agent-log-uk.png');

    await open('Налаштування');
    await choose('Тема', 'dark');
    await open('Родина');
    await browser.waitUntil(async () => (await $$('span[title^="Агент змінив"]')).length > 0, { timeoutMsg: 'no badge in Ukrainian' });
    await browser.pause(800);
    await shot('agent-family-uk-dark.png');
    expect(await browser.execute(() => document.documentElement.getAttribute('data-theme'))).toBe('dark');

    await click('Змінити особу');
    expect(await panelCount()).toMatch(/^\d+\/\d+$/);
    await browser.pause(400);
    await shot('agent-panel-uk-dark.png');

    // Back to English and the light theme for the rest.
    await open('Налаштування');
    await choose('Тема', 'light');
    await choose('Мова', 'en');
    await $('h1=Settings').waitForDisplayed({ timeout: 20000 });
  });

  it('adds nothing when the same file is worked through again', async () => {
    const people = await client.people();
    const rows = await client.logTotal();
    await client.work(home, living);
    expect(await client.people()).toBe(people);
    expect(await client.logTotal()).toBe(rows);
  });

  it('folds the panel on the person page and counts what is left to review', async () => {
    await open('Family');
    await click('Edit person');
    const count = await panelCount();
    const [left, total] = count.split('/').map(Number);
    expect(count).toMatch(/^\d+\/\d+$/);
    expect(left).toBe(total);
    expect(await panelOpen()).toBe(false);
    await browser.pause(400);
    await shot('agent-panel-folded-en.png');

    await $('//summary[contains(.,"What an agent changed")]').click();
    expect(await panelOpen()).toBe(true);
    await browser.waitUntil(async () => (await text()).includes(CHILD), { timeoutMsg: 'the open panel does not list the change' });
    await browser.pause(400);
    await shot('agent-panel-open-en.png');
  });

  it('clears the badges when the batch is marked reviewed', async () => {
    await open('Change log');
    await $(`h2*=${TITLE}`).waitForDisplayed();
    const review = await $('//section[contains(@class,"batch")]//header//button[normalize-space(.)="Mark reviewed"]');
    await review.waitForClickable();
    await review.click();
    await browser.waitUntil(async () => (await $$('//section[contains(@class,"batch")]//header//button[normalize-space(.)="Mark reviewed"]')).length === 0, {
      timeoutMsg: 'the batch still offers Mark reviewed',
    });
    await open('Family');
    // The person first: on a page still loading there are no badges either, which proves nothing.
    await browser.waitUntil(async () => (await text()).includes(CHILD), { timeoutMsg: 'the daughter is not on the family page' });
    await browser.waitUntil(async () => (await badges()) === 0, { timeoutMsg: 'badges stayed after the review' });

    // All reviewed: the panel gives just the total.
    await click('Edit person');
    expect(await panelCount()).toMatch(/^\d+$/);
    await browser.pause(400);
    await shot('agent-panel-reviewed-en.png');
  });

  it('puts the tree back when the batch is undone', async () => {
    await open('Change log');
    await $(`h2*=${TITLE}`).waitForDisplayed();
    await click('Undo batch');
    // `*=` matches links only: a dialog's title needs its own text selector.
    const dialog = '//*[self::dialog or @role="dialog"]';
    await $(`${dialog}//*[contains(normalize-space(.),"Undo this batch?")]`).waitForDisplayed();
    await $(`${dialog}//button[normalize-space(.)="Undo"]`).click();
    await $(`${dialog}//*[contains(normalize-space(.),"Undo finished")]`).waitForDisplayed({ timeout: 30000 });
    await click('Done');

    expect(await client.people()).toBe(peopleBefore);
    await open('Family');
    await browser.waitUntil(async () => !(await text()).includes(CHILD), { timeoutMsg: 'the daughter is still on screen after the undo' });
    await clearFocus();
    await search(CHILD);
    await browser.pause(800);
    expect((await $$(HITS)).length).toBe(0);
    await browser.keys('Escape');
  });

  it('stops the agent at once when the connection is revoked', async () => {
    await open('Settings');
    await (await $(`aria/Revoke ${AGENT}`)).click();
    await click('Revoke access');
    await $(`aria/Revoke ${AGENT}`).waitForExist({ reverse: true, timeout: 20000 });
    expect((await client.rpc('tools/list', {})).status).toBe(401);
    // Its name stays in the log.
    await open('Change log');
    await $(`h2*=${TITLE}`).waitForDisplayed();
    expect(await text()).toContain(`By ${AGENT}`);
  });
});
