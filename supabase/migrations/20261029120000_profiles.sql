-- Profil de chaque compte : ce que le serveur doit savoir de l'utilisateur,
-- en plus de son compte (auth.users). Pour l'instant : profession de santé,
-- qui désactive la synchronisation tant qu'il n'y a pas d'hébergement HDS
-- (D-37).
--
-- Règle de minim pour chaque table : RLS activé et forcé, règles explicites,
-- GRANT explicites, rien pour le rôle `anon` (supabase/README.md).

create table public.profiles (
  id uuid primary key references auth.users (id) on delete cascade,
  health_profession boolean not null default false,
  created_at timestamptz not null default now()
);

alter table public.profiles enable row level security;
alter table public.profiles force row level security;

revoke all on table public.profiles from anon, authenticated;
grant select on table public.profiles to authenticated;
grant update (health_profession) on table public.profiles to authenticated;

create policy "profil : lecture par son titulaire"
  on public.profiles for select
  to authenticated
  using ((select auth.uid()) = id);

create policy "profil : modification par son titulaire"
  on public.profiles for update
  to authenticated
  using ((select auth.uid()) = id)
  with check ((select auth.uid()) = id);

-- Création du profil à l'inscription, à partir des métadonnées envoyées par
-- l'application (`health_profession`). Toute autre valeur que `true` donne
-- `false`.
create function public.handle_new_user()
returns trigger
language plpgsql
security definer
set search_path = ''
as $$
begin
  insert into public.profiles (id, health_profession)
  values (new.id, coalesce(new.raw_user_meta_data ->> 'health_profession' = 'true', false));
  return new;
end;
$$;

revoke all on function public.handle_new_user() from public, anon, authenticated;

create trigger on_auth_user_created
  after insert on auth.users
  for each row execute function public.handle_new_user();

-- Suppression du compte par son titulaire (droit à l'effacement, RGPD). Le
-- profil suit (on delete cascade).
create function public.delete_my_account()
returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
  current_user_id uuid := auth.uid();
begin
  if current_user_id is null then
    raise exception 'authentification requise' using errcode = '42501';
  end if;
  delete from auth.users where id = current_user_id;
end;
$$;

revoke all on function public.delete_my_account() from public, anon;
grant execute on function public.delete_my_account() to authenticated;
