-- L'option « Enable automatic RLS » des projets hébergés crée la fonction
-- `public.rls_auto_enable()`, appelée par un déclencheur d'événement à chaque
-- création de table. Comme toute fonction du schéma public, elle reste
-- exécutable par `anon` et `authenticated` via l'API (/rest/v1/rpc), ce que
-- signalent les Security Advisors. Le déclencheur n'a pas besoin de ce droit.
--
-- La fonction n'existe pas en local (option propre aux projets hébergés),
-- d'où la vérification.
do $$
begin
  if to_regprocedure('public.rls_auto_enable()') is not null then
    revoke all on function public.rls_auto_enable() from public, anon, authenticated;
  end if;
end;
$$;
