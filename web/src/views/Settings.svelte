<script lang="ts">
  import { ChevronRight, Users } from '@lucide/svelte';
  import Page from '../components/Page.svelte';
  import * as Card from '$lib/components/ui/card';
  import { auth } from '$lib/store.svelte';
  import Account from './settings/Account.svelte';
  import Calendars from './settings/Calendars.svelte';
  import Channels from './settings/Channels.svelte';
  import Data from './settings/Data.svelte';
  import General from './settings/General.svelte';
</script>

<Page title="设置" class="max-w-3xl">
  <div class="grid gap-4">
    <General />
    <Channels />
    <Calendars />
    <Data />

    {#if auth.user?.is_admin}
      <a href="#/admin" class="block">
        <Card.Root class="hover:ring-primary/40 transition-shadow">
          <Card.Header>
            <Card.Title class="flex items-center gap-2"><Users class="size-4" />用户管理</Card.Title>
            <Card.Description>创建账号、重置密码、停用或删除用户、修改注册策略、备份数据库</Card.Description>
            <Card.Action><ChevronRight class="text-muted-foreground size-4" /></Card.Action>
          </Card.Header>
        </Card.Root>
      </a>
    {/if}

    <Account />
  </div>
</Page>
