-- Sécurité de la base (D-37) : tests pgTAP, lancés par `supabase test db` en
-- CI. Une table sans RLS, ou lisible par un autre utilisateur, fait échouer
-- la CI même si tout le reste passe.

begin;
create extension if not exists pgtap with schema extensions;
select plan(12);

-- 1. Aucune table d'un schéma exposé par l'API sans RLS activé et forcé.
select is_empty(
  $$ select n.nspname || '.' || c.relname
     from pg_class c join pg_namespace n on n.oid = c.relnamespace
     where c.relkind in ('r', 'p')
       and n.nspname in ('public', 'graphql_public')
       and not (c.relrowsecurity and c.relforcerowsecurity) $$,
  'chaque table exposée a le RLS activé et forcé'
);

-- 2. Le rôle anonyme n'a aucun droit sur les tables exposées.
select is_empty(
  $$ select table_schema || '.' || table_name || ' : ' || privilege_type
     from information_schema.role_table_grants
     where grantee = 'anon' and table_schema in ('public', 'graphql_public') $$,
  'le rôle anon n''a aucun droit sur les tables'
);

-- 3. Le rôle anonyme ne peut appeler aucune fonction de public.
select is_empty(
  $$ select p.proname from pg_proc p join pg_namespace n on n.oid = p.pronamespace
     where n.nspname = 'public' and has_function_privilege('anon', p.oid, 'execute') $$,
  'le rôle anon ne peut appeler aucune fonction'
);

-- Deux comptes : Alice (profession de santé) et Bob.
insert into auth.users (id, email, raw_user_meta_data, aud, role)
values
  ('00000000-0000-0000-0000-00000000000a', 'alice@example.com', '{"health_profession": true}', 'authenticated', 'authenticated'),
  ('00000000-0000-0000-0000-00000000000b', 'bob@example.com', '{"health_profession": "oui"}', 'authenticated', 'authenticated');

-- 4. Le profil est créé à l'inscription, avec la profession déclarée.
select results_eq(
  $$ select health_profession from public.profiles order by id $$,
  $$ values (true), (false) $$,
  'profil créé à l''inscription ; seule la valeur true compte'
);

-- 5. Anonyme : aucune lecture.
set local role anon;
select throws_ok($$ select * from public.profiles $$, '42501', null, 'anon ne lit pas les profils');
reset role;

-- Bob connecté.
set local role authenticated;
select set_config('request.jwt.claims', '{"sub": "00000000-0000-0000-0000-00000000000b", "role": "authenticated"}', true);

-- 6. Bob ne voit que son profil.
select results_eq($$ select id::text from public.profiles $$, $$ values ('00000000-0000-0000-0000-00000000000b') $$, 'Bob ne voit que son profil');

-- 7. Bob ne peut pas modifier le profil d'Alice (aucune ligne touchée).
select results_eq(
  $$ with changed as (update public.profiles set health_profession = false where id = '00000000-0000-0000-0000-00000000000a' returning 1) select count(*)::int from changed $$,
  $$ values (0) $$,
  'Bob ne modifie pas le profil d''Alice'
);

-- 8. Bob peut modifier sa profession.
select results_eq(
  $$ with changed as (update public.profiles set health_profession = true where id = '00000000-0000-0000-0000-00000000000b' returning 1) select count(*)::int from changed $$,
  $$ values (1) $$,
  'Bob modifie sa profession'
);

-- 9. Bob ne peut pas changer l'identifiant ni insérer de profil.
select throws_ok($$ update public.profiles set id = gen_random_uuid() $$, '42501', null, 'identifiant non modifiable');
select throws_ok($$ insert into public.profiles (id) values (gen_random_uuid()) $$, '42501', null, 'pas d''insertion directe');

-- 10. Bob supprime son compte ; son profil disparaît, celui d'Alice reste.
select lives_ok($$ select public.delete_my_account() $$, 'Bob supprime son compte');
reset role;
select results_eq($$ select id::text from public.profiles $$, $$ values ('00000000-0000-0000-0000-00000000000a') $$, 'seul le profil d''Alice reste');

select * from finish();
rollback;
