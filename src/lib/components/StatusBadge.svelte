<script lang="ts">
  import { id } from "$lib/i18n/id";
  import type { MeetingStatus } from "$lib/types";

  let {
    status,
    progressDone = 0,
    progressTotal = 0,
  }: { status: MeetingStatus; progressDone?: number; progressTotal?: number } = $props();

  const withCount = $derived(
    (status === "transcribing" || status === "summarizing") && progressTotal > 0
      ? ` (${progressDone}/${progressTotal})`
      : "",
  );

  const color = $derived(
    status === "done"
      ? "bg-emerald-100 text-emerald-800"
      : status === "failed"
        ? "bg-red-100 text-red-800"
        : status === "recording"
          ? "bg-red-600 text-white"
          : status === "interrupted"
            ? "bg-amber-100 text-amber-900"
            : status === "waiting_quota" || status === "waiting_network"
              ? "bg-orange-100 text-orange-900"
              : "bg-indigo-100 text-indigo-800",
  );
</script>

<span class={["inline-flex shrink-0 items-center rounded-full px-2.5 py-0.5 text-xs font-medium whitespace-nowrap", color]}>
  {id.status[status]}{withCount}
</span>
