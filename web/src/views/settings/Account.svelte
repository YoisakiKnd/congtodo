<script lang="ts">
  import { LogOut } from '@lucide/svelte';
  import { toast } from 'svelte-sonner';
  import Field from '../../components/Field.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import { Input } from '$lib/components/ui/input';
  import { auth, changePassword, signOut } from '$lib/store.svelte';

  let pw = $state({ old: '', next: '', confirm: '' });
  async function submit(e: SubmitEvent) {
    e.preventDefault();
    if (pw.next !== pw.confirm) return toast.error('两次输入的新密码不一致');
    try {
      await changePassword(pw.old, pw.next);
      pw = { old: '', next: '', confirm: '' };
      toast.success('密码已修改，其他设备需要重新登录');
    } catch (err) {
      toast.error((err as Error).message);
    }
  }
</script>

<Card.Root>
  <Card.Header>
    <Card.Title>账号</Card.Title>
    <Card.Description>
      当前登录：<b class="text-foreground">{auth.user?.username}</b>{#if auth.user?.is_admin}<Badge variant="secondary" class="ml-2">管理员</Badge>{/if}
    </Card.Description>
    <Card.Action>
      <Button variant="outline" size="sm" onclick={signOut}><LogOut />退出登录</Button>
    </Card.Action>
  </Card.Header>
  <Card.Content>
    <form class="grid gap-4" onsubmit={submit}>
      <div class="grid gap-3 sm:grid-cols-3">
        <Field label="原密码"><Input required type="password" autocomplete="current-password" bind:value={pw.old} /></Field>
        <Field label="新密码"><Input required minlength={8} type="password" autocomplete="new-password" bind:value={pw.next} /></Field>
        <Field label="确认新密码"><Input required type="password" autocomplete="new-password" bind:value={pw.confirm} /></Field>
      </div>
      <div><Button type="submit" size="sm" variant="outline">修改密码</Button></div>
    </form>
  </Card.Content>
</Card.Root>
