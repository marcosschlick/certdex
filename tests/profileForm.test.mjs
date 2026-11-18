import test from "node:test";
import assert from "node:assert/strict";
import { createComponent } from "./componentHarness.mjs";

test("requires all basic profile fields during editing as well as setup", async () => {
  for (const setup of [false, true]) {
    const profile = {
      name: "Name", email: "name@example.com", phone: "123", city: "City", state: "State", country: "Country",
      github: "", linkedin: "", orcid: "", website: "", professional_summary: "", academic_summary: "", documents_dir: "",
    };
    let savedProfile;
    let saves = 0;
    const view = createComponent("Profile", (command, args) => {
      if (command === "get_profile") return Promise.resolve(profile);
      if (command === "save_profile") { saves += 1; savedProfile = args.profile; return Promise.resolve(); }
      throw new Error(command);
    }, true, { setup });
    view.find(() => false);
    await new Promise(setImmediate);
    const input = (label) => view.find((node) => node.type === "label" && node.props.children?.[0]?.props?.children === label).props.children[1];
    for (const label of ["Name", "Email", "Phone", "City", "State", "Country"]) {
      assert.equal(input(label).props.required, true);
      for (const value of ["", "  \t "]) {
        input(label).props.onChange({ target: { value } });
        await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
        assert.equal(saves, 0);
        assert.equal(view.find((node) => node.type === "p" && node.props.role === "status").props.children, "Please complete all required profile fields.");
      }
      input(label).props.onChange({ target: { value: profile[label.toLowerCase()] } });
    }
    assert.equal(input("Email").props.type, "email");
    if (!setup) {
      input("Documents folder").props.onChange({ target: { value: "~/CustomCerts" } });
    }
    await view.find((node) => node.type === "form").props.onSubmit({ preventDefault() {} });
    assert.equal(saves, 1);
    assert.equal(savedProfile.documents_dir, setup ? "" : "~/CustomCerts");
  }
});

test("profile language selector defaults to system language, saves locally, and updates UI text", async () => {
  const profile = {
    name: "Name", email: "name@example.com", phone: "123", city: "City", state: "State", country: "Country",
    github: "", linkedin: "", orcid: "", website: "", professional_summary: "", academic_summary: "", documents_dir: "",
  };
  const view = createComponent(
    "Profile",
    (command) => {
      if (command === "get_profile") return Promise.resolve(profile);
      throw new Error(command);
    },
    true,
    { setup: false },
    { navigator: { languages: ["pt-BR"] } },
  );
  view.find(() => false);
  await new Promise(setImmediate);

  const languageSelect = () => view.find((node) => node.type === "select");
  assert.equal(languageSelect().props.value, "pt-BR");
  assert.equal(view.find((node) => node.type === "h1").props.children, "Perfil");

  languageSelect().props.onChange({ target: { value: "es-UY" } });
  assert.equal(view.storage.getItem("certdex.locale"), "es-UY");
  assert.equal(view.find((node) => node.type === "button" && node.props.type === "submit").props.children, "Guardar cambios");
});
