<script lang="ts">
  // 开关（原生 Switch 的移植）：轨道 36×20、拇指 16、状态键在 aria-checked 上。
  // 只表达「开/关」；多选请用 checkbox —— 一个开关无法表达"选了 3 个"。
  interface Props {
    checked: boolean;
    onchange: (next: boolean) => void;
    label: string;
    disabled?: boolean;
  }

  let { checked, onchange, label, disabled = false }: Props = $props();
</script>

<button
  type="button"
  role="switch"
  class="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  onclick={() => onchange(!checked)}
>
  <span class="thumb"></span>
</button>
