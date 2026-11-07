// Backend text in other languages: every sentence the Rust core wrote in its
// own tests (src/i18n/catalog/samples.txt, recorded with
// FREEBOARD_I18N_SAMPLES) comes out translated. Names, paths and commands
// match no template and stay as they are.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { setLang, type Lang } from "../../src/i18n/index.ts";
import { tb, tbKnown } from "../../src/i18n/backend.ts";

const samples = readFileSync(new URL("../../src/i18n/catalog/samples.txt", import.meta.url), "utf8").split("\n").filter(Boolean);
/** Text that is a name, a path, a command or a size: never translated. */
const verbatim = (s: string) =>
  /^(~\/|\/|[A-Z]:\\)/.test(s) || /^[\w.:@-]+$/.test(s) || /^(docker|git|pnpm|npm|pip3?|brew|xcrun|cargo) /.test(s) || /^Xcode DerivedData$/.test(s) || / did not answer within 20 seconds\.$/.test(s);

for (const l of ["nl", "de", "fr", "es"] as Lang[]) {
  test(`${l}: the core's sentences come out translated`, () => {
    setLang(l);
    // Every sentence is in the catalog (a name inside it may stay as is)...
    const unknown = samples.filter((s) => !verbatim(s) && !tbKnown(s, l as Exclude<Lang, "en">));
    // ...and nearly all of them read differently once translated.
    const changed = samples.filter((s) => !verbatim(s) && tb(s) !== s).length;
    const total = samples.filter((s) => !verbatim(s)).length;
    setLang("en");
    assert.deepEqual(unknown, [], `${unknown.length} sentences have no translation`);
    assert.ok(changed / total > 0.8, `only ${changed} of ${total} sentences changed`);
  });
}

test("translated text keeps names and local sizes", () => {
  setLang("de");
  assert.match(tb("Volume billing_pgdata"), /billing_pgdata/);
  assert.equal(tb("~/projects/web/node_modules"), "~/projects/web/node_modules");
  assert.equal(tb("docker image rm postgres:14"), "docker image rm postgres:14");
  setLang("fr");
  assert.match(tb("I can read ~/Library/Mail and 2 more places now. Scan again to see what is inside."), /~\/Library\/Mail/);
  setLang("en");
  assert.equal(tb("Created 71 days ago"), "Created 71 days ago");
});
