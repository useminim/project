# Corpus PDF de test

Documents **entièrement synthétiques** : noms, adresses, montants et IBAN sont inventés. Aucun document réel de client ne doit être ajouté ici.

| Fichier | Contenu | Origine |
| --- | --- | --- |
| `facture.pdf` | Facture d'une page avec un tableau | `sources/facture.html` imprimé en PDF par Microsoft Edge (headless) |
| `police.pdf` | Attestation en polices Gabriola et Comic Sans (polices incorporées) | `sources/police.html`, Edge |
| `scan.pdf` | « Scan » de la facture : une image seule, sans texte (2 pages) | Capture d'écran de `facture.html` insérée dans `sources/scan.html`, Edge |
| `vide.pdf` | Une page blanche, sans texte | `sources/vide.html`, Edge |
| `chiffre.pdf` | `facture.pdf` chiffré en AES-128, mot de passe utilisateur « secret » | Outil `encrypt/` (lopdf) |

Les cas « PDF tronqué » et « fichier qui n'est pas un PDF » sont construits dans les tests à partir de ces fichiers.

## Régénérer

Depuis WSL ou Git Bash sous Windows (Microsoft Edge requis) :

```bash
fixtures/pdf/generate.sh
```

Les PDF régénérés peuvent différer octet par octet (dates de création) ; les tests ne vérifient que le texte, le nombre de pages et les erreurs.
