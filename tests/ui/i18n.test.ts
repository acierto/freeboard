import { test } from "node:test";
import assert from "node:assert/strict";
import en from "../../src/i18n/en.json" with { type: "json" };
import nl from "../../src/i18n/nl.json" with { type: "json" };
import de from "../../src/i18n/de.json" with { type: "json" };
import fr from "../../src/i18n/fr.json" with { type: "json" };
import es from "../../src/i18n/es.json" with { type: "json" };
import { ago, firstSupported, fx, resolveLang, setLang, t, tf, unitGB } from "../../src/i18n/index.ts";
import { itemName } from "../../src/i18n/backend.ts";

test("every language has every English key (spec: CI fails otherwise)", () => {
  for (const [l, m] of Object.entries({ nl, de, fr, es })) assert.deepEqual(Object.keys(en).filter((k) => !(k in m)), [], l);
});

test("placeholders survive translation", () => {
  const holes = (s: string) => [...s.matchAll(/\{[a-z]+\}/g)].map((m) => m[0]).sort().join();
  for (const [l, m] of Object.entries({ nl, de, fr, es }) as [string, Record<string, string>][])
    for (const [k, v] of Object.entries(en as Record<string, string>)) assert.equal(holes(m[k]), holes(v), `${l} ${k}`);
});

test("system language: the first supported preference, else English", () => {
  assert.equal(firstSupported(["pt-BR", "de-AT", "en"]), "de");
  assert.equal(firstSupported(["ja"]), "en");
  assert.equal(resolveLang("system", "fr"), "fr");
  assert.equal(resolveLang("es", "fr"), "es");
});

test("numbers and units follow the language", () => {
  setLang("de");
  assert.equal(fx(12.4) + " " + unitGB(), "12,4 GB");
  assert.equal(t("h.toss", { x: "54,0 GB" }), "54,0 GB über Bord");
  setLang("fr");
  assert.equal(unitGB(), "Go");
  assert.equal(ago(18), "il y a 18 min");
  setLang("en");
  assert.equal(ago(18), "18 min ago");
  assert.equal(ago(120), "2 hours ago");
  assert.equal(tf("n.unknown-id", "My item"), "My item");
});

test("item names: fixed descriptive titles translate, named ones never do", () => {
  setLang("de");
  assert.equal(itemName({ id: "docker.build_cache:x", title: "Docker build cache" }), "Docker-Build-Cache");
  assert.equal(itemName({ id: "docker.volumes:pg", title: "Volume pgdata" }), "Volume pgdata");
  assert.equal(itemName({ id: "b-node", title: "node_modules in quiet projects" }), t("n.b-node"));
  assert.notEqual(t("n.b-node"), "node_modules in quiet projects");
  setLang("en");
});
