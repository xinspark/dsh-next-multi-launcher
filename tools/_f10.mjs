import fs from 'node:fs';

// 1) SettingsDialog：显示当前权限
let p = 'src/lib/SettingsDialog.svelte';
let t = fs.readFileSync(p, 'utf8');
if (!t.includes('settings.permission')) {
  t = t.replace(
    `  <div class="field">
    <label class="check">`,
    `  <div class="field">
    <div class="field-label"><span>{t('settings.permission')}</span></div>
    <span class="field-hint">{t('settings.standard')} / {t('settings.admin')}（{isAdminLabel()}）</span>
  </div>

  <div class="field">
    <label class="check">`
  );
  t = t.replace(
    `  const defaultHomePreview = $derived(`,
    `  function isAdminLabel(): string {
    return t('settings.standard');
  }

  const defaultHomePreview = $derived(`
  );
  fs.writeFileSync(p, t);
  console.log('settings dialog updated');
}

// 2) App：支持 #settings 直接打开设置（截图/调试用）
p = 'src/App.svelte';
t = fs.readFileSync(p, 'utf8');
if (!t.includes("location.hash === '#settings'")) {
  t = t.replace(
    `    await refresh();
    pollTimer = setInterval(refresh, 2000);`,
    `    await refresh();
    if (location.hash === '#settings') dialog = { kind: 'settings' };
    pollTimer = setInterval(refresh, 2000);`
  );
  fs.writeFileSync(p, t);
  console.log('hash hook added');
}
