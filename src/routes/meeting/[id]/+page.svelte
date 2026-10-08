<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import MeetingDetail from "$lib/components/MeetingDetail.svelte";
  import { viewport } from "$lib/viewport.svelte";

  const meetingId = $derived(page.params.id ?? "");
  const tab = $derived(page.url.searchParams.get("tab"));

  // Layar lebar: detail tampil di panel kanan Beranda (daftar tetap terlihat).
  onMount(() => {
    if (viewport.wide) goto(`/?m=${meetingId}${tab ? `&tab=${tab}` : ""}`, { replaceState: true });
  });
</script>

<MeetingDetail {meetingId} initialTab={tab} />
