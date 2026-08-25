import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const SRC = resolve(__dirname, '../src');
const lines = (path: string) => readFileSync(resolve(SRC, path), 'utf-8').split(/\r?\n/).length;

describe('settings module architecture guards', () => {
  it('keeps static dialog markup outside settings-ui orchestration', () => {
    const source = readFileSync(resolve(SRC, 'settings-ui.ts'), 'utf-8');
    expect(source).toContain("from './settings/panel-template'");
    expect(source).not.toContain('<div class="settings-box">');
  });

  it('prevents settings-ui from returning to its previous 2100+ line size', () => {
    expect(lines('settings-ui.ts')).toBeLessThan(2000);
    expect(lines('settings/panel-template.ts')).toBeLessThan(300);
  });
});
