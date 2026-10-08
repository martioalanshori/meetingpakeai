<script lang="ts">
  import { id } from "$lib/i18n/id";
  import type { MeetingStatus } from "$lib/types";

  // Status sebagai teks + titik warna (bukan pil berwarna): daftar tetap tenang, yang perlu perhatian menonjol.
  let {
    status,
    progressDone = 0,
    progressTotal = 0,
  }: { status: MeetingStatus; progressDone?: number; progressTotal?: number } = $props();

  const withCount = $derived(
    (status === "transcribing" || status === "summarizing") && progressTotal > 0
      ? ` ${progressDone}/${progressTotal}`
      : "",
  );

  const tone = $derived(
    status === "done"
      ? { dot: "bg-ok", text: "text-ink-soft" }
      : status === "failed"
        ? { dot: "bg-bad", text: "text-bad font-semibold" }
        : status === "recording"
          ? { dot: "bg-rec motion-safe:animate-pulse", text: "text-rec font-semibold" }
          : status === "interrupted" || status === "waiting_quota" || status === "waiting_network"
            ? { dot: "bg-warn", text: "text-warn font-semibold" }
            : { dot: "bg-ink motion-safe:animate-pulse", text: "text-ink font-medium" },
  );
</script>

<span class={["tabular inline-flex shrink-0 items-center gap-1.5 text-sm whitespace-nowrap", tone.text]}>
  <span class={["h-2 w-2 rounded-full", tone.dot]} aria-hidden="true"></span>
  {id.status[status]}{withCount}
</span>
