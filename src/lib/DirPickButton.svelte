<script lang="ts">
  // 「选择目录」的唯一实现：三处入口（设置/初始化向导/版本本地导入）共用一个按钮，
  // 免得同一个动作在三个地方各写一遍 api.pickDirectory + 各自一套禁用/忙碌文案。
  import { api } from './api';

  interface Props {
    label: string;
    busyLabel?: string;
    disabled?: boolean;
    /** 拿到用户选中的目录（已确认非空）；可以是异步，按钮在它完成前保持忙碌态 */
    onpick: (dir: string) => void | Promise<void>;
    /** 取消失败 / 保存失败时回传错误，由调用方决定怎么显示 */
    onerror?: (err: unknown) => void;
  }

  let { label, busyLabel, disabled = false, onpick, onerror }: Props = $props();

  let picking = $state(false);

  async function pick() {
    picking = true;
    try {
      const dir = await api.pickDirectory();
      if (dir) await onpick(dir);
    } catch (err) {
      onerror?.(err);
    } finally {
      picking = false;
    }
  }
</script>

<button class="btn ghost" disabled={disabled || picking} onclick={pick}>
  {picking ? (busyLabel ?? label) : label}
</button>
