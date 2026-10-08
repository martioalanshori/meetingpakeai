<script lang="ts">
  import { untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import MeetingDetail from "$lib/components/MeetingDetail.svelte";
  import { detailTab, viewport } from "$lib/viewport.svelte";

  const meetingId = $derived(page.params.id ?? "");
  const tab = $derived(page.url.searchParams.get("tab"));
  const at = $derived(Number(page.url.searchParams.get("at") ?? "NaN"));

  // Jendela cukup lebar (juga saat diperbesar) → detail tampil di panel kanan halaman Meeting; tab aktif dipertahankan.
  $effect(() => {
    if (!viewport.wide) return;
    const current = detailTab.meetingId === meetingId ? detailTab.tab : tab;
    untrack(() => goto(`/meetings?m=${meetingId}${current ? `&tab=${current}` : ""}`, { replaceState: true }));
  });
</script>

<MeetingDetail {meetingId} initialTab={tab} initialMs={Number.isFinite(at) ? at : null} />
