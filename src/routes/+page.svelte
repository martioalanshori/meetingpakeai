<script lang="ts">
  // Beranda (tampilan pertama). Isi lengkap dibuat di langkah B3 (PLAN-beranda.md).
  import { id } from "$lib/i18n/id";
</script>

<main class="mx-auto flex w-full max-w-2xl flex-col gap-6 px-6 pt-7 pb-12">
  <h1 class="text-2xl font-bold tracking-[-0.02em]">{id.nav.home}</h1>
  <a href="/meetings" class="link self-start">{id.nav.meetings}</a>
</main>
