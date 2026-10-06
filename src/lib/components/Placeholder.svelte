<script lang="ts">
  import { id } from "$lib/i18n/id";

  let { title }: { title: string } = $props();
</script>

<main class="mx-auto flex max-w-4xl flex-col gap-4 p-6">
  <a href="/" class="text-sm text-indigo-700 hover:underline">{id.placeholder.back}</a>
  <h1 class="text-xl font-semibold">{title}</h1>
  <p class="text-gray-600">{id.placeholder.comingSoon}</p>
</main>
