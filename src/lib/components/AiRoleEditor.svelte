<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "$lib/api";
  import ClipboardKeyHint from "$lib/components/ClipboardKeyHint.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Menu from "$lib/components/Menu.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import { id } from "$lib/i18n/id";
  import type { AiConfig, AiRole, AppError, Endpoint } from "$lib/types";

  // Penyedia + model + API key untuk satu peran (Transkrip / Ringkasan).
  let { role, config, onsaved }: { role: AiRole; config: AiConfig; onsaved: () => void } = $props();

  const t = id.ai;

  // Draf diambil dari konfigurasi tersimpan saat komponen dibuat / konfigurasi dimuat ulang.
  let draft = $state<Endpoint>({ provider: "", baseUrl: "", model: "" });
  let keyInput = $state("");
  let changingKey = $state(false);
  let busy = $state(false);
  let message = $state<{ kind: "ok" | "warn" | "error"; text: string } | null>(null);

  $effect(() => {
    const saved = role === "stt" ? config.stt : config.llm;
    draft = { provider: saved.provider, baseUrl: saved.baseUrl, model: saved.model };
    keyInput = "";
    changingKey = false;
  });

  const saved = $derived(role === "stt" ? config.stt : config.llm);
  const presets = $derived(config.presets.filter((p) => (role === "stt" ? p.sttModel !== null : p.llmModel !== null)));
  const preset = $derived(config.presets.find((p) => p.id === draft.provider));
  const keySet = $derived(config.keysSet.includes(draft.provider));
  const showKeyInput = $derived(!keySet || changingKey);
  const dirty = $derived(
    draft.provider !== saved.provider ||
      draft.model.trim() !== saved.model ||
      (draft.provider === "custom" && draft.baseUrl.trim() !== saved.baseUrl) ||
      keyInput.trim() !== "",
  );

  async function changeProvider(e: Event) {
    const provider = (e.currentTarget as HTMLSelectElement).value;
    message = null;
    keyInput = "";
    changingKey = false;
    try {
      draft = await api.defaultAiEndpoint(role, provider);
    } catch (err) {
      message = { kind: "error", text: (err as AppError).message };
    }
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    busy = true;
    message = null;
    try {
      const res = await api.saveAiEndpoint(role, draft, keyInput.trim() || null);
      message = res.modelMissing
        ? { kind: "warn", text: t.modelMissing(draft.model) }
        : !res.verified
          ? { kind: "ok", text: t.savedUnverified }
          : { kind: "ok", text: t.saved };
      onsaved();
    } catch (err) {
      message = { kind: "error", text: (err as AppError).message };
    } finally {
      busy = false;
    }
  }

  async function removeKey() {
    const ok = await confirmDialog({
      title: t.removeConfirm(preset?.name ?? draft.provider),
      message: t.removeMessage,
      confirmText: t.removeKey,
      danger: true,
    });
    if (!ok) return;
    try {
      await api.deleteAiKey(draft.provider);
      message = { kind: "ok", text: t.removed };
      onsaved();
    } catch (err) {
      message = { kind: "error", text: (err as AppError).message };
    }
  }
</script>

<form class="flex flex-col gap-4" onsubmit={save}>
  <div class="flex flex-col gap-0.5">
    <h3 class="label">{role === "stt" ? t.sttTitle : t.llmTitle}</h3>
    <p class="hint">{role === "stt" ? t.sttHint : t.llmHint}</p>
  </div>

  <div class="grid gap-3 sm:grid-cols-2">
    <label class="flex flex-col gap-1.5">
      <span class="label">{t.provider}</span>
      <span class="relative">
        <select class="field w-full appearance-none pr-9" value={draft.provider} onchange={changeProvider}>
          {#each presets as p (p.id)}
            <option value={p.id}>{p.name}</option>
          {/each}
        </select>
        <Icon name="chevron-down" size={16} class="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2 text-ink-soft" />
      </span>
    </label>
    <label class="flex flex-col gap-1.5">
      <span class="label">{t.model}</span>
      <input class="field" spellcheck="false" bind:value={draft.model} placeholder={t.modelPlaceholder} />
    </label>
  </div>

  {#if draft.provider === "custom"}
    <label class="flex flex-col gap-1.5">
      <span class="label">{t.baseUrl}</span>
      <input class="field" spellcheck="false" bind:value={draft.baseUrl} placeholder="http://localhost:11434/v1" />
      <span class="hint">{t.baseUrlHint}</span>
    </label>
  {/if}

  <div class="flex flex-col gap-1.5">
    <div class="flex flex-wrap items-center gap-x-3 gap-y-1">
      <span class="label">{t.apiKey}</span>
      <span class={["flex items-center gap-1.5 text-sm", keySet ? "text-ok" : "text-ink-soft"]}>
        <span class={["h-2 w-2 rounded-full", keySet ? "bg-ok" : "bg-ink-faint"]} aria-hidden="true"></span>
        {keySet ? t.keySaved : preset?.keyRequired === false ? t.keyOptional : t.keyMissing}
      </span>
      {#if keySet && !changingKey}
        <div class="ml-auto">
          <Menu
            label={t.keyActions}
            triggerClass="btn btn-quiet btn-sm"
            items={[
              { label: t.changeKey, icon: "pencil", onselect: () => (changingKey = true) },
              { label: t.removeKey, icon: "trash", danger: true, onselect: removeKey },
            ]}
          >
            {#snippet trigger()}{t.keyActions}<Icon name="chevron-down" size={14} />{/snippet}
          </Menu>
        </div>
      {/if}
    </div>
    {#if showKeyInput}
      {#if keyInput.trim() === "" && preset?.keyPrefix}
        <ClipboardKeyHint provider={draft.provider} onuse={(k) => (keyInput = k)} />
      {/if}
      <input
        type="password"
        autocomplete="off"
        spellcheck="false"
        class="field"
        placeholder={preset?.keyPrefix ? `${preset.keyPrefix}…` : t.keyPlaceholder}
        aria-label={t.apiKey}
        bind:value={keyInput}
      />
      {#if preset?.keyUrl}
        <button type="button" class="link self-start text-sm" onclick={() => openUrl(preset!.keyUrl!)}>
          {t.getKey(preset.name)}
        </button>
      {/if}
    {/if}
  </div>

  <div class="flex flex-wrap items-center gap-3">
    <button type="submit" class="btn btn-ink" disabled={busy || !dirty}>
      {busy ? t.testing : t.testAndSave}
    </button>
    {#if !dirty && !busy && !message}
      <span class="hint">{t.nothingToTest}</span>
    {/if}
    {#if message}
      <p
        role="status"
        class={["text-sm", message.kind === "ok" && "text-ok", message.kind === "warn" && "text-warn", message.kind === "error" && "text-bad"]}
      >
        {message.text}
      </p>
    {/if}
  </div>
</form>
