# Licences tierces

Composants tiers intégrés dans l'application distribuée dont la licence impose de reproduire une mention ou le texte de la licence. La liste complète des dépendances est vérifiée en CI par `cargo deny` (licences permissives uniquement).

## SQLCipher

Chiffrement de la base locale (SQLCipher Community Edition, compilé par la crate `libsqlite3-sys`). Licence BSD à trois clauses :

```
Copyright (c) 2008-2020 Zetetic LLC
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:
    * Redistributions of source code must retain the above copyright
      notice, this list of conditions and the following disclaimer.
    * Redistributions in binary form must reproduce the above copyright
      notice, this list of conditions and the following disclaimer in the
      documentation and/or other materials provided with the distribution.
    * Neither the name of the ZETETIC LLC nor the
      names of its contributors may be used to endorse or promote products
      derived from this software without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY ZETETIC LLC ''AS IS'' AND ANY
EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL ZETETIC LLC BE LIABLE FOR ANY
DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## OpenSSL

Primitives cryptographiques utilisées par SQLCipher (OpenSSL 4, compilé depuis les sources par la crate `openssl-src`). Copyright The OpenSSL Project Authors. Distribué sous la licence Apache 2.0, dont le texte complet est dans [`licenses/Apache-2.0.txt`](licenses/Apache-2.0.txt).

## SQLite

SQLite, sur lequel repose SQLCipher, est dans le domaine public.
