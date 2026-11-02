<script lang="ts">
  // 原生图标的唯一渲染口：16×16 网格、fill:none、描边继承 currentColor。
  // 线宽走属性（Regular 1 / Medium 1.3），与官方的两种字重对应。
  import { ICONS, type IconName } from './icons';

  interface Props {
    name: IconName;
    size?: number;
    width?: number;
  }

  let { name, size = 16, width = 1 }: Props = $props();
  const paths = $derived(ICONS[name]);
</script>

<svg
  class="svg-icon"
  width={size}
  height={size}
  viewBox="0 0 16 16"
  fill="none"
  aria-hidden="true"
  stroke-width={width}
  stroke-linecap="round"
  stroke-linejoin="round"
>
  {#each paths as d (d)}
    <path {d} stroke="currentColor" />
  {/each}
</svg>
