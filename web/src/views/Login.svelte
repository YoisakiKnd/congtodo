<script lang="ts">
  import { CheckSquare } from '@lucide/svelte';
  import Field from '../components/Field.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import * as Tabs from '$lib/components/ui/tabs';
  import { auth, signIn } from '$lib/store.svelte';

  // 还没有用户时直接进入注册（创建管理员）
  let mode = $state<'login' | 'register'>(auth.first ? 'register' : 'login');
  let username = $state('');
  let password = $state('');
  let confirm = $state('');
  let error = $state('');
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = '';
    if (mode === 'register' && password !== confirm) {
      error = '两次输入的密码不一致';
      return;
    }
    busy = true;
    try {
      await signIn(username.trim(), password, mode === 'register');
    } catch (err) {
      error = (err as Error).message;
    } finally {
      busy = false;
    }
  }
</script>

<div class="bg-muted/40 flex min-h-dvh items-center justify-center p-4">
  <div class="grid w-full max-w-sm gap-6">
    <div class="flex items-center justify-center gap-2.5 text-xl font-semibold">
      <span class="bg-primary text-primary-foreground grid size-9 place-items-center rounded-xl">
        <CheckSquare class="size-5" />
      </span>
      Congtodo
    </div>

    <Card.Root>
      <Card.Header>
        <Card.Title class="text-lg">
          {mode === 'login' ? '登录' : auth.first ? '创建管理员账号' : '注册'}
        </Card.Title>
        <Card.Description>
          {#if auth.first}
            这是第一个账号，将成为管理员
          {:else if mode === 'login'}
            欢迎回来
          {:else}
            创建一个新账号
          {/if}
        </Card.Description>
      </Card.Header>
      <Card.Content>
        {#if auth.signup && !auth.first}
          <Tabs.Root bind:value={mode} class="mb-5">
            <Tabs.List class="w-full">
              <Tabs.Trigger value="login" class="flex-1">登录</Tabs.Trigger>
              <Tabs.Trigger value="register" class="flex-1">注册</Tabs.Trigger>
            </Tabs.List>
          </Tabs.Root>
        {/if}

        <form class="grid gap-4" onsubmit={submit}>
          <Field label="用户名">
            <Input required autocomplete="username" bind:value={username} />
          </Field>
          <Field label="密码" hint={mode === 'register' ? '至少 8 位' : undefined}>
            <Input
              required
              type="password"
              minlength={mode === 'register' ? 8 : undefined}
              autocomplete={mode === 'register' ? 'new-password' : 'current-password'}
              bind:value={password}
            />
          </Field>
          {#if mode === 'register'}
            <Field label="确认密码">
              <Input required type="password" autocomplete="new-password" bind:value={confirm} />
            </Field>
          {/if}
          {#if error}<p class="text-destructive text-sm">{error}</p>{/if}
          <Button type="submit" size="lg" class="mt-1 w-full" disabled={busy}>
            {mode === 'login' ? '登录' : '创建账号'}
          </Button>
        </form>
      </Card.Content>
    </Card.Root>

    {#if !auth.signup && !auth.first}
      <p class="text-muted-foreground text-center text-xs">本站暂不开放注册，请联系管理员</p>
    {/if}
  </div>
</div>
