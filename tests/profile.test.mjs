import test from "node:test";
import assert from "node:assert/strict";
import { isProfileComplete, requiredProfileFields } from "../src/profile.ts";

const completeProfile = {
  name: "Name",
  email: "name@example.com",
  phone: "Phone",
  city: "City",
  state: "State",
  country: "Country",
};

test("requires every basic profile field", () => {
  assert.equal(isProfileComplete(completeProfile), true);
  for (const field of requiredProfileFields) {
    assert.equal(isProfileComplete({ ...completeProfile, [field]: "" }), false);
    assert.equal(isProfileComplete({ ...completeProfile, [field]: "   " }), false);
  }
});
