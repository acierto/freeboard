// Task 2.3: Dinghy's deck height and mood for the spec's free-space points.
// Run with `npm run test:ui` (Node's built-in test runner).
import { test } from "node:test";
import assert from "node:assert/strict";
import { boatTransform, boatY, crateLabels, MAX_CRATES } from "../../src/components/dinghyGeometry.ts";
import { moodOf, rawStatus } from "../../src/features/home/homeModel.ts";

const GB = 1e9;
const thresholds = { lowThresholdBytes: 20 * GB, criticalThresholdBytes: 8 * GB };

test("deck height moves in a straight line from 0 to 60 GB free, then stays still", () => {
  assert.equal(boatY(0), 212);
  assert.equal(boatY(4), 209.1);
  assert.equal(boatY(12.4), 202.9);
  assert.equal(boatY(40), 182.7);
  assert.equal(boatY(60), 168);
  assert.equal(boatY(120), 168, "above 60 GB the boat stays at its highest");
  assert.equal(boatY(-1), 212, "never below the empty position");
  // Equal steps in free space move the boat equal distances below 60 GB
  // (within the prototype's rounding to 0.1 scene units).
  for (let g = 0; g < 60; g += 5) assert.ok(Math.abs(boatY(g) - boatY(g + 5) - (44 * 5) / 60) <= 0.1, `step at ${g} GB`);
});

test("the transform matches the prototype's boatAt()", () => {
  assert.equal(boatTransform(12.4), "translate(292px,202.9px)");
  assert.equal(boatTransform(120), "translate(292px,168.0px)");
});

test("mood follows status: happy, worried, panicked", () => {
  const at = (g: number) => moodOf(rawStatus(g * GB, thresholds));
  assert.equal(at(0), "crit");
  assert.equal(at(4), "crit");
  assert.equal(at(7.9), "crit");
  assert.equal(at(8), "warn");
  assert.equal(at(12.4), "warn");
  assert.equal(at(19.9), "warn");
  assert.equal(at(20), "good");
  assert.equal(at(40), "good");
  assert.equal(at(60), "good");
  assert.equal(at(120), "good");
});

test("up to nine crates, each labeled with its rounded GB", () => {
  assert.deepEqual(crateLabels([18.7, 14.8, 11.3, 9.2]), [19, 15, 11, 9]);
  assert.equal(crateLabels(Array.from({ length: 12 }, () => 1)).length, MAX_CRATES);
});
