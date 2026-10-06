<script lang="ts">
  import { id } from "$lib/i18n/id";
</script>

<main class="mx-auto flex h-full max-w-4xl flex-col gap-6 p-6">
  <header class="flex items-center justify-between">
    <h1 class="text-xl font-semibold">{id.appName}</h1>
    <div class="flex items-center gap-2">
      <!-- Langkah 6: tombol membuka popup consent, lalu start_recording. -->
      <button
        type="button"
        class="rounded-lg bg-red-600 px-5 py-2.5 font-medium text-white hover:bg-red-700"
      >
        {id.home.startRecording}
      </button>
      <a
        href="/settings"
        class="rounded-lg px-3 py-2.5 text-gray-700 hover:bg-gray-200"
        aria-label={id.home.settings}
        title={id.home.settings}
      >
        ⚙
      </a>
    </div>
  </header>

  <section class="flex flex-1 items-center justify-center rounded-xl border border-dashed border-gray-300 bg-white p-10 text-center text-gray-600">
    {id.home.empty}
  </section>
</main>
