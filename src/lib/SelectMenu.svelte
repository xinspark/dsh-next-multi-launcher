<script lang="ts">
  // 下拉选择器（原生 DSH 设置里的 select）。
  // 闭合态 = 原生 registryToggle 的形状（32px / border-l4 / radius-md / bg-layer-2），
  // 展开态 = 原生 registry 浮层 + registryOption 行（radius-md / padding 8-10 / 选中行加粗并带对勾）。
  // 不用原生 <select>：它的下拉列表无法上样式，而"选择器开合态"正是这次要对齐的部分。
  import Icon from './Icon.svelte';

  interface Option {
    value: string;
    label: string;
    disabled?: boolean;
  }

  interface Props {
    value: string;
    options: Option[];
    onchange: (value: string) => void;
    disabled?: boolean;
    label?: string;
  }

  let { value, options, onchange, disabled = false, label }: Props = $props();

  // 每个实例一个锚名，否则同页两个选择器会互相抢位置
  const uid = `picker-${Math.random().toString(36).slice(2, 9)}`;
  const current = $derived(options.find((o) => o.value === value));

  function choose(next: string, unavailable?: boolean) {
    document.getElementById(uid)?.hidePopover();
    if (unavailable || next === value) return;
    onchange(next);
  }
</script>

<button
  type="button"
  class="picker"
  style="anchor-name: --{uid}"
  popovertarget={uid}
  {disabled}
  aria-haspopup="menu"
  aria-label={label}
>
  <span class="picker-value">{current?.label ?? value}</span>
  <Icon name="chevron-down" size={12} width={1.2} />
</button>

<div id={uid} popover class="picker-menu" role="menu" style="position-anchor: --{uid}">
  {#each options as option (option.value)}
    <button
      type="button"
      class="picker-option"
      role="menuitemradio"
      aria-checked={option.value === value}
      aria-disabled={option.disabled ?? false}
      disabled={option.disabled}
      onclick={() => choose(option.value, option.disabled)}
    >
      <span class="picker-option-label">{option.label}</span>
      {#if option.value === value}
        <Icon name="check" width={1.5} />
      {/if}
    </button>
  {/each}
</div>
