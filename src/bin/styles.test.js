const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const postcss = require('postcss');

for (const surface of ['src', 'src-web']) {
  test(`${surface} sidebar keeps clear/settings button styles after removing Live2D`, () => {
    const file = path.resolve(__dirname, `../renderer/${surface}/css/sidebar-workspace.css`);
    const css = postcss.parse(fs.readFileSync(file, 'utf8'), { from: file });
    for (const selector of ['.clear-btn', '.settings-btn']) {
      const declarations = {};
      css.walkRules(rule => {
        if (rule.selectors.includes(selector)) rule.walkDecls(decl => { declarations[decl.prop] = decl.value; });
      });
      assert.equal(declarations.height, '28px', `${selector} should retain its hit area`);
      assert.equal(declarations['border-radius'], 'var(--radius-sm)', `${selector} should retain its button appearance`);
    }
  });
}
