---
name: topic
description: Workflow obligatoire pour tout nouveau sujet sur Fourchette (fonctionnalité, amélioration ou bug) : analyse, issue GitHub dans le Project Fourchette, worktree, développement, tests, test visuel, test du dev au terminal, puis commit, push, PR, statut du Project et rangement. À utiliser dès que le dev demande d'ajouter, de changer ou de corriger quelque chose, ou tape /topic. Pas pour une simple question, ni pour continuer un sujet déjà en cours.
---

# Topic : un nouveau sujet

Chaque sujet suit cet ordre, sans en sauter une étape. **Un sujet = une issue = un worktree = une PR.** Si le dev glisse un deuxième sujet en route, on le note et on le traite après, séparément.

C'est le `topic` de Warhallow (rally-point), avec GitHub Issues et le Project **Fourchette** à la place de Linear, et les outils de Rust à la place de ceux de Godot. Les règles du dépôt (commandes, architecture, issues, version) sont dans `CLAUDE.md`.

Montre l'avancement avec les chapitres de la section suivante, et rien d'autre : ni barre de progression, ni liste du parcours dans tes messages. Si une étape est incomplète, on ne passe pas à la suivante.

## Avancement

Les huit étapes portent toujours ces noms courts : Analyse, GitHub, Worktree, Développement, Tests, Visuel, Test au terminal, Clôture.

À chaque début d'étape, marque un chapitre avec `mcp__ccd_session__mark_chapter`, au titre exact `N/8 Étape` (par exemple `5/8 Tests`). Le titre ne porte rien d'autre : ni numéro d'issue, ni parenthèse, et pas de `summary`.

- **Si une étape bloque**, ne change pas de chapitre : dis en une phrase ce qui bloque, avec la sortie.
- **Si on revient en arrière** (un test du dev échoue à l'étape 7, on corrige à l'étape 4), marque de nouveau le chapitre de l'étape où l'on revient.
- **Sans cet outil** (hors de l'app Claude), écris seulement `N/8 Étape` en première ligne du message, au début de chaque étape.

## Niveau du sujet : modèle et effort

Chaque sujet a un niveau, Simple, Standard ou Complexe, qui choisit le modèle et l'effort des sessions à part de ses étapes. Le niveau se règle à l'étape 1 et s'annonce en une ligne (« Niveau : Standard, Sonnet à effort medium pour le développement »). Le dev peut le changer à tout moment, et un sujet qui révèle en route un risque (perte de travail, état du dépôt) monte de niveau, annoncé de la même façon.

**Critères**, le premier qui convient l'emporte :

- **Complexe** : touche ce qui peut perdre du travail ou abîmer un dépôt (`undo.rs` et les `Snapshot`, `worktree.rs`, la suppression ou le reset dans `git.rs`, une commande Git qui écrit), les fils et canaux (`App::spawn`, `ai_poll`, `project_poll`), ce qui est écrit chez l'utilisateur (`library.rs`, `repos.json`), ce qui est envoyé à Claude ou à GitHub (`ai.rs`, `github.rs`), un bug dont on ne trouve pas la cause, ou plus de six fichiers de code.
- **Standard** : une fonctionnalité ou un bug localisé, de trois à six fichiers de code, qui n'écrit rien dans le dépôt ouvert.
- **Simple** : docs, skill, outillage, textes, une constante ou une couleur, un ou deux fichiers.

**Le modèle et l'effort par étape** (`model` et `effort` de la session à part, `mcp__ccd_session__start_session`) :

| Étape | Simple | Standard | Complexe |
|---|---|---|---|
| 1 Analyse (lecture du code) | Haiku, low | Haiku, low | Sonnet, medium |
| 2 GitHub | session principale | session principale | session principale |
| 3 Worktree | session principale | session principale | session principale |
| 4 Développement | Haiku, low | Sonnet, medium | Opus, high |
| 5 Tests (tri des échecs) | session principale | Haiku, low | Sonnet, medium |
| 6 Visuel | session principale | session principale | session principale |
| 7 Test au terminal | session principale (le dev essaie) | session principale | session principale |
| 8 Commit, PR, Project | session principale | session principale | session principale |

Règles de délégation :

- **Déléguer, c'est ouvrir une session à part, jamais lancer un agent en arrière-plan** : une étape déléguée part dans un sous-chat que le dev voit dans la barre latérale, peut ouvrir, suivre et arrêter (`mcp__ccd_session__start_session`, `initiation` « user_asked », `context` « fresh », `use_worktree` false avec un brief qui dit de travailler dans le worktree du sujet). Un agent en arrière-plan (l'outil Agent, `run_in_background`) ne se lance que si le dev le demande explicitement.
- **Une session à part ne voit pas la conversation** : son brief dit le but, le worktree et les fichiers, ce qui est déjà fait, ce qui est exclu, ce qu'il doit rendre et ce qu'il ne fait pas (ni commit, ni push, ni tests « live »).
- **L'issue, le choix des tests, l'essai du dev, le commit et la PR restent dans la session principale**, qui reprend la main quand la session à part a fini.
- **Le rapport d'une session à part est une indication, pas une preuve** : relis le diff, les fichiers et lignes cités, les sorties de tests avant de t'en servir.
- **Le coût suit le niveau** : Opus ne se prend que pour le niveau Complexe. Pour un doute sur le niveau, prends le plus bas et monte si la tâche le demande.

## 1. Analyser

Lis le code et les docs concernés (`CLAUDE.md`, `app/MECHANICS.md` si le sujet touche une valeur mesurée sur Fork, les fichiers de `app/src/` touchés). Reformule le sujet en deux ou trois phrases. Aucun code. Classe le sujet selon « Niveau du sujet » et annonce le niveau.

## 2. Chercher dans GitHub

Cherche d'abord une issue existante, ouverte ou fermée, dans le dépôt et dans le Project :

```bash
gh issue list --repo C2-POLY/fourchette --state all --search "<mots>" --limit 30
gh project item-list 1 --owner C2-POLY --format json --limit 300 --jq '.items[] | "\(.status)\t#\(.content.number // "-")\t\(.title)"'
```

Puis :

- **Elle existe** : lis-la (`gh issue view <n> --repo C2-POLY/fourchette --comments`), résume-la au dev, vérifie que ses critères d'acceptation sont encore justes. Une issue fermée et livrée ne se rouvre pas : si le sujet la reprend, on définit une nouvelle issue qui la cite (« Reprend #12 »).
- **Elle n'existe pas** : on la définit ensemble, **avant tout code**. Pose au dev les questions ouvertes : comportement attendu, cas limites, ce qui est exclu, ce qui peut être perdu ou écrit dans un dépôt. Ne devine pas. Rédige l'issue selon « Work items » de `CLAUDE.md` (français, titre `<Zone> : …`, critères en cases à cocher, type, label `zone:`), fais-la valider, puis crée-la et mets-la dans le Project :

```bash
# Le type (Feature, Task, Bug) se donne à la création par l'API ; --body-file évite les problèmes de guillemets.
url=$(gh api repos/C2-POLY/fourchette/issues -f title="<titre>" -F body=@<fichier du corps> -f type=Task -f 'labels[]=zone:<zone>' -f "assignees[]=$(gh api user --jq .login)" --jq .html_url)
# Garde l'id de l'item : `item-list` ne montre pas tout de suite un item qui vient d'être ajouté.
item=$(gh project item-add 1 --owner C2-POLY --url "$url" --format json --jq .id)
```

Un label `zone:` qui manque se crée (`gh label create zone:<zone> --repo C2-POLY/fourchette`) plutôt que d'être omis. Un sujet découpé est une Feature avec des sous-issues ; une dépendance est une relation `blocked by`, pas une phrase.

**Changer un statut** (les identifiants sont dans `CLAUDE.md`, « Work items ») :

```bash
# Sans id gardé à l'étape 2 (issue ajoutée plus tôt), cherche-le dans le Project.
[ -n "$item" ] || item=$(gh project item-list 1 --owner C2-POLY --format json --limit 300 --jq ".items[] | select(.content.number == <n>) | .id")
gh project item-edit --project-id PVT_kwDODd-WqM4BqKPF --id "$item" --field-id PVTSSF_lADODd-WqM4BqKPFzhoijpg --single-select-option-id <option>
```

## 3. Worktree

Toujours un worktree, et tu ne demandes rien au dev, ni le choix ni le nom : plusieurs sessions Claude peuvent partager le dossier principal et changer la branche sous tes pieds. Choisis le nom toi-même, `feat/<n>-…` pour une Feature, `fix/<n>-…` pour un Bug, `chore/<n>-…` pour l'outillage et la doc (`<n>` le numéro de l'issue), et crée-le à côté du dossier principal, depuis `origin/main` et sans qu'il suive `main`, pour qu'un `git push` nu ne parte jamais vers `main` :

```bash
git fetch --prune && git worktree add -b <type>/<n>-<nom> ../fourchette-<nom> origin/main && git -C ../fourchette-<nom> branch --unset-upstream
```

Un worktree neuf n'a pas de `app/target/` : copie celui du dossier principal pour éviter une compilation complète, `cp -Rc <dossier principal>/app/target ../fourchette-<nom>/app/` (instantané sur APFS). Passe l'issue à **In progress**, dis en une ligne le chemin et la branche, puis passe à l'étape 4. Avant chaque commit, revérifie la branche avec `git branch --show-current`.

## 4. Travailler

Respecte `CLAUDE.md` (architecture, `Cmd` et `Dialog`, fils et canaux, icônes dessinées, version). Toute action qui change le dépôt ouvert prend un `Snapshot` avant, pour que Cmd+Z la reprenne. Ajoute ou mets à jour les tests unitaires (`#[cfg(test)]` dans le fichier touché ; un test qui appelle Claude ou GitHub est `#[ignore]`). Quand le sujet change quelque chose de visible dans l'app, applique la règle de version du jour (« Versioning »). Si une valeur de `app/mechanics.json` change, mets `app/MECHANICS.md` à jour. Le développement se délègue au modèle et à l'effort du tableau de « Niveau du sujet ».

## 5. Tester

Depuis `app/` du worktree :

```bash
# Compiler : aucun nouveau warning par rapport à main
cargo build 2>&1 | grep -E '^(warning|error)'

# Tests unitaires : tous doivent passer
cargo test

# Lints : pas plus que les 5 warnings de main (code mort de git.rs et app.rs, deux remarques de style)
cargo clippy 2>&1 | grep -E '^(warning|error)' | sort | uniq -c

# Seulement si le sujet touche ai.rs ou github.rs : les tests qui appellent Claude Code et GitHub pour de vrai
cargo test -- --ignored --nocapture
```

Un test qui échoue s'explique par le sujet ou se corrige ; ne le désactive pas. Un warning nouveau se corrige. En cas de doute, lance la même commande dans le dossier principal, sur `main`, pour comparer.

## 6. Tester visuellement

Vérifie ce que le sujet change à l'écran : capture du projet d'exemple, lancée depuis un dossier qui n'est pas un dépôt, puis lis l'image.

```bash
S=<scratchpad>
(cd /tmp && <worktree>/app/target/debug/fourchette --screenshot "$S/now.png")
(cd /tmp && FOURCHETTE_SWITCHER=1 <worktree>/app/target/debug/fourchette --screenshot "$S/switcher.png")
```

`FOURCHETTE_PALETTE="<texte>"`, `FOURCHETTE_SWITCHER=1` et `FOURCHETTE_CLEANUP_PREVIEW=1` ouvrent la palette, le sélecteur de dépôts et la fenêtre de nettoyage. Si le sujet touche une vue qu'aucune n'ouvre, ajoute un réglage du même genre dans `App::new`. Recadre avec `sips -c <h> <w> --cropOffset <y> <x>` pour regarder un détail. Contrôle les glyphes (un carré veut dire que la police ne l'a pas), le thème sombre et les textes coupés. Dis ce que tu as vu et ce que tu n'as pas pu voir (un menu contextuel, un survol, un vrai dépôt).

## 7. Faire tester le dev, avec lecture du terminal

Ne lance pas l'app toi-même pour lui. Donne au dev la commande pour la lancer **depuis le worktree**, en un seul bloc `bash`, plus une ligne sur ce qu'il doit essayer :

```bash
cargo run --manifest-path <chemin absolu du worktree>/app/Cargo.toml -- <dépôt sur lequel essayer>
```

Le chemin est celui du worktree, vérifié avec `git worktree list` avant l'envoi : ne reprends jamais celui d'une commande précédente. Si son terminal montre un autre chemin, dis-le tout de suite.

Quand il a essayé et fermé l'app, lis son terminal (outil `read_terminal`, onglet principal ; sinon demande-lui de coller la sortie). Cherche les `panicked`, les `error`, les `warning` du build et tout message inattendu, puis demande-lui ce qu'il a vu : un terminal propre ne dit pas ce qu'il a vu, donc attends son verdict avant de commiter. Corrige et reteste jusqu'à ce que tout soit propre.

## 8. Clôturer, seulement si aucune erreur

1. Commit sur la branche du sujet (message en anglais, Conventional Commits : `feat(switcher): …`, sans numéro de version). Jamais de relecture du diff par un agent avant le commit : ses tests et l'essai du dev sont le filet.
2. Push de la branche, puis PR vers `main` (titre en anglais, Conventional Commits ; corps avec `Closes #<n>`). L'issue passe à **In review**.
3. Issue : coche les critères que le dev a vérifiés à l'étape 7, ajoute la ligne `**Livré** : \`vX.Y.Z\`` et le label de version. Une fois la PR mergée, le Project passe l'issue à **Done**. Si un critère n'a pas pu être testé, l'issue va à **To test** avec sa case décochée, et tu dis lequel.
4. Une fois la PR mergée dans `main`, déplace le tag de version (voir « Versioning » de `CLAUDE.md`).
5. **Worktree du sujet, sans le redemander.** Dès que la PR est mergée, supprime le worktree du sujet et sa branche locale. Vérifie d'abord, depuis le dossier principal et non depuis le worktree à supprimer :
   - `git fetch --prune`, puis la branche est bien dans `origin/main` (`git branch -r --merged origin/main`, ou la PR « merged » si elle a été squashée) ;
   - `git -C <worktree> status --short` ne montre rien.

   Puis `git worktree remove <chemin du worktree>` et `git branch -D <branche>` (`-D` seulement si la PR a été squashée et que GitHub la dit mergée). Si le worktree a du travail non commité ou si la branche n'est pas mergée, ne supprime rien : dis ce qui bloque, avec la sortie, et demande.
6. **Rangement du reste, en un seul message.** Propose tout ce qui reste, jamais la moitié :
   - la branche distante du sujet, si elle existe encore (`git fetch --prune`) ;
   - `main` du dossier principal à jour (`git merge --ff-only origin/main`, seulement s'il est sur `main` et propre) ;
   - tes fichiers temporaires (captures, journaux du scratchpad). Écris `"${S:?}"/*.png` plutôt qu'une variable nue ;
   - les autres worktrees et branches locales déjà mergés dans `main` : regarde le `git status` de chacun, et ne touche jamais à un dossier avec du travail non commité ni à une branche distante qui n'est pas la tienne ;
   - une issue (Backlog) pour chaque défaut remarqué en route et laissé de côté ;
   - les issues liées encore ouvertes alors que leur travail est fini.

S'il reste une erreur, on ne commit pas et on ne ferme pas l'issue : tu dis ce qui échoue, avec la sortie.
