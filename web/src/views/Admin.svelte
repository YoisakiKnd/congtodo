<script lang="ts">
  import { Ellipsis, KeyRound, Plus, ShieldCheck, ShieldOff, Trash2, UserCheck, UserX } from '@lucide/svelte';
  import { onMount } from 'svelte';
  import { toast } from 'svelte-sonner';
  import Field from '../components/Field.svelte';
  import Page from '../components/Page.svelte';
  import Sheet from '../components/Sheet.svelte';
  import { Badge } from '$lib/components/ui/badge';
  import { Button, buttonVariants } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import { Input } from '$lib/components/ui/input';
  import { Switch } from '$lib/components/ui/switch';
  import * as Table from '$lib/components/ui/table';
  import * as ToggleGroup from '$lib/components/ui/toggle-group';
  import { api } from '$lib/api';
  import { auth, clock } from '$lib/store.svelte';
  import { fmtRelative } from '$lib/time';
  import type { AdminUser, SignupMode } from '$lib/types';
  import { cn } from '$lib/utils';

  let users = $state<AdminUser[]>([]);
  let signup = $state<SignupMode>('first');
  let signupDefault = $state<SignupMode>('first');
  const now = $derived(new Date(clock.minute));

  const SIGNUP: { value: SignupMode; label: string; desc: string }[] = [
    { value: 'first', label: '仅首个账号', desc: '只有还没有任何用户时允许注册' },
    { value: 'open', label: '开放注册', desc: '任何人都可以注册账号' },
    { value: 'closed', label: '关闭注册', desc: '只能由管理员创建账号' },
  ];

  const err = (e: unknown) => toast.error((e as Error).message);

  async function load() {
    try {
      const [list, s] = await Promise.all([
        api.get<AdminUser[]>('/admin/users'),
        api.get<{ signup: SignupMode; signup_default: SignupMode }>('/admin/settings'),
      ]);
      users = list;
      signup = s.signup;
      signupDefault = s.signup_default;
    } catch (e) {
      err(e);
    }
  }
  onMount(load);

  async function setSignup(v: string) {
    if (!v || v === signup) return;
    try {
      await api.put('/admin/settings', { signup: v });
      signup = v as SignupMode;
      toast.success('注册策略已更新');
    } catch (e) {
      err(e);
    }
  }

  /** 修改用户并用返回的最新数据替换列表中的这一行 */
  async function patch(u: AdminUser, body: Record<string, unknown>, done: string) {
    const row = await api.patch<AdminUser>(`/admin/users/${u.id}`, body);
    users = users.map((x) => (x.id === u.id ? row : x));
    toast.success(done);
  }

  async function remove(u: AdminUser) {
    const detail = u.items ? `及其全部 ${u.items} 条数据` : '';
    if (!confirm(`确定删除用户「${u.username}」${detail}？此操作不可恢复。`)) return;
    try {
      await api.del(`/admin/users/${u.id}`);
      users = users.filter((x) => x.id !== u.id);
      toast.success(`已删除「${u.username}」`);
    } catch (e) {
      err(e);
    }
  }

  // ---------- 新建用户 ----------
  let createOpen = $state(false);
  let form = $state({ username: '', password: '', is_admin: false });

  function openCreate() {
    form = { username: '', password: '', is_admin: false };
    createOpen = true;
  }

  async function create() {
    try {
      const row = await api.post<AdminUser>('/admin/users', form);
      users = [...users, row];
      toast.success(`已创建「${row.username}」`);
    } catch (e) {
      err(e);
      throw e;
    }
  }

  // ---------- 重置密码 ----------
  let resetOpen = $state(false);
  let resetUser = $state<AdminUser | null>(null);
  let resetPw = $state('');

  function openReset(u: AdminUser) {
    resetUser = u;
    resetPw = '';
    resetOpen = true;
  }

  async function reset() {
    try {
      await patch(resetUser!, { password: resetPw }, `已重置「${resetUser!.username}」的密码，其所有设备需重新登录`);
    } catch (e) {
      err(e);
      throw e;
    }
  }

  const act = (u: AdminUser, body: Record<string, unknown>, done: string) => patch(u, body, done).catch(err);
</script>

<Page title="用户管理" sub="仅管理员可见" class="max-w-5xl">
  {#snippet actions()}
    <Button onclick={openCreate}><Plus />新建用户</Button>
  {/snippet}

  <div class="grid gap-4">
    <Card.Root>
      <Card.Header>
        <Card.Title>注册策略</Card.Title>
        <Card.Description>
          {SIGNUP.find((x) => x.value === signup)?.desc}。部署时的默认值（CONGTODO_SIGNUP）为「{SIGNUP.find((x) => x.value === signupDefault)?.label}」，在这里修改后立即生效并以此为准。
        </Card.Description>
      </Card.Header>
      <Card.Content>
        <ToggleGroup.Root type="single" variant="outline" value={signup} onValueChange={setSignup}>
          {#each SIGNUP as s (s.value)}
            <ToggleGroup.Item value={s.value}>{s.label}</ToggleGroup.Item>
          {/each}
        </ToggleGroup.Root>
        {#if signup === 'open'}
          <p class="text-muted-foreground mt-3 text-xs">
            提示：开放注册后，任何人注册即可配置 Webhook，让服务器向任意地址发请求，请只对信任的人开放。
          </p>
        {/if}
      </Card.Content>
    </Card.Root>

    <Card.Root class="py-0">
      <Table.Root>
        <Table.Header>
          <Table.Row>
            <Table.Head class="pl-6">用户</Table.Head>
            <Table.Head class="hidden sm:table-cell">注册时间</Table.Head>
            <Table.Head>最近登录</Table.Head>
            <Table.Head class="hidden text-right md:table-cell">数据</Table.Head>
            <Table.Head class="hidden text-right md:table-cell">在线设备</Table.Head>
            <Table.Head class="w-12 pr-4"></Table.Head>
          </Table.Row>
        </Table.Header>
        <Table.Body>
          {#each users as u (u.id)}
            {@const self = u.id === auth.user?.id}
            <Table.Row class={cn(u.disabled && 'opacity-55')}>
              <Table.Cell class="pl-6">
                <div class="flex items-center gap-2">
                  <span
                    class="bg-primary/15 text-primary grid size-7 shrink-0 place-items-center rounded-full text-xs font-semibold uppercase"
                  >
                    {u.username.slice(0, 1)}
                  </span>
                  <span class="font-medium">{u.username}</span>
                  {#if self}<Badge variant="outline">你</Badge>{/if}
                  {#if u.is_admin}<Badge variant="secondary">管理员</Badge>{/if}
                  {#if u.disabled}<Badge variant="destructive">已停用</Badge>{/if}
                </div>
              </Table.Cell>
              <Table.Cell class="text-muted-foreground hidden sm:table-cell">{fmtRelative(u.created_at, now, false)}</Table.Cell>
              <Table.Cell class="text-muted-foreground">
                {u.last_login_at ? fmtRelative(u.last_login_at, now) : '从未'}
              </Table.Cell>
              <Table.Cell class="num hidden text-right md:table-cell">{u.items}</Table.Cell>
              <Table.Cell class="num hidden text-right md:table-cell">{u.sessions}</Table.Cell>
              <Table.Cell class="pr-4 text-right">
                <DropdownMenu.Root>
                  <DropdownMenu.Trigger class={buttonVariants({ variant: 'ghost', size: 'icon-sm' })} aria-label="操作">
                    <Ellipsis />
                  </DropdownMenu.Trigger>
                  <DropdownMenu.Content align="end" class="w-40">
                    <DropdownMenu.Item onSelect={() => openReset(u)}><KeyRound />重置密码</DropdownMenu.Item>
                    {#if !self}
                      {#if u.is_admin}
                        <DropdownMenu.Item onSelect={() => act(u, { is_admin: false }, `已取消「${u.username}」的管理员身份`)}>
                          <ShieldOff />取消管理员
                        </DropdownMenu.Item>
                      {:else}
                        <DropdownMenu.Item onSelect={() => act(u, { is_admin: true }, `已将「${u.username}」设为管理员`)}>
                          <ShieldCheck />设为管理员
                        </DropdownMenu.Item>
                      {/if}
                      {#if u.disabled}
                        <DropdownMenu.Item onSelect={() => act(u, { disabled: false }, `已启用「${u.username}」`)}>
                          <UserCheck />启用账号
                        </DropdownMenu.Item>
                      {:else}
                        <DropdownMenu.Item onSelect={() => act(u, { disabled: true }, `已停用「${u.username}」，其设备已全部下线`)}>
                          <UserX />停用账号
                        </DropdownMenu.Item>
                      {/if}
                      <DropdownMenu.Separator />
                      <DropdownMenu.Item variant="destructive" onSelect={() => remove(u)}><Trash2 />删除用户</DropdownMenu.Item>
                    {/if}
                  </DropdownMenu.Content>
                </DropdownMenu.Root>
              </Table.Cell>
            </Table.Row>
          {/each}
        </Table.Body>
      </Table.Root>
    </Card.Root>
    <p class="text-muted-foreground text-xs">
      停用账号或重置密码后，该用户所有设备立即下线；停用期间不会收到任何提醒。删除用户会一并删除其全部数据。
    </p>
  </div>
</Page>

<Sheet bind:open={createOpen} title="新建用户" onsave={create}>
  <Field label="用户名" hint="3-32 位字母、数字、下划线或横线">
    <Input required autocomplete="off" bind:value={form.username} />
  </Field>
  <Field label="初始密码" hint="至少 8 位，请通过安全的方式告诉对方">
    <Input required minlength={8} autocomplete="new-password" bind:value={form.password} />
  </Field>
  <label class="flex items-center gap-3 text-sm"><Switch bind:checked={form.is_admin} />设为管理员</label>
</Sheet>

<Sheet bind:open={resetOpen} title="重置「{resetUser?.username}」的密码" onsave={reset}>
  <Field label="新密码" hint="至少 8 位；重置后该用户所有设备需要重新登录">
    <Input required minlength={8} autocomplete="new-password" bind:value={resetPw} />
  </Field>
</Sheet>
