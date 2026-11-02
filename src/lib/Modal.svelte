<script lang="ts">
  // 通用模态外壳：遮罩 + 卡片 + 标题栏。内容用 snippet 传入。
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';

  interface Props {
    title: string;
    onClose: () => void;
    children: Snippet;
  }

  let { title, onClose, children }: Props = $props();

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') onClose();
  }
</script>

<svelte:window on:keydown={onKeydown} />

<div
  class="backdrop"
  role="presentation"
  onclick={(event) => {
    if (event.target === event.currentTarget) onClose();
  }}
>
  <div class="modal" role="dialog" aria-modal="true" aria-label={title}>
    <div class="modal-head">
      <span>{title}</span>
      <span class="spacer"></span>
      <button class="btn icon tiny" aria-label="close" onclick={onClose}
        ><Icon name="close" width={1.5} /></button
      >
    </div>
    <div class="modal-body">
      {@render children()}
    </div>
  </div>
</div>
