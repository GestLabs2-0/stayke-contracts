# Stayke Contracts — Documentación as implemented

Guías de implementación de los programas Anchor en este repo. Complementan la **SoT** de producto/arquitectura en stayke-docs; no la sustituyen.

## Glosario SoT


| Término                   | Significado                                                                                                                                                                      |
| ------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **SoT** (Source of Truth) | Norma de producto y arquitectura: repo hermano [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md) (ECONOMIC-MODEL, ADRs, System Design, PRD).                |
| **As implemented**        | Lo que el código on-chain hace hoy en `programs/`**. Puede divergir de la SoT.                                                                                                   |
| **Gap**                   | Diferencia explícita entre política SoT y un gate/comportamiento on-chain. Se documenta con callout «Policy SoT vs On-chain gate»; el arreglo de código va en otras iteraciones. |


Si SoT y as-implemented chocan: **manda la SoT** para producto; estas guías solo describen el código y el gap.

## Audiencia


| Quién                  | Qué busca aquí                                             |
| ---------------------- | ---------------------------------------------------------- |
| On-chain / Anchor      | Instrucciones, PDAs, CPI reales, constraints               |
| Backend / integradores | Flujos firmables, gates que fallan on-chain                |
| Producto               | Qué ya está enforced vs qué sigue siendo solo política SoT |




## Qué vive dónde


| Tema                                 | Dónde                                                                                                                                                                                                             |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bond / tiers / L1–L6, escrow vs bond | SoT: [ECONOMIC-MODEL](https://github.com/GestLabs2-0/docs/blob/main/architecture/ECONOMIC-MODEL.md), [ADR-007](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-007-bond-escrow-separation.md) |
| Yield / lending Stage 2              | SoT: [ADR-010](https://github.com/GestLabs2-0/docs/blob/main/architecture/adrs/ADR-010-yield-deferred-stage-2.md)                                                                                                 |
| System Design normativo              | SoT: [stayke-docs](https://github.com/GestLabs2-0/docs/blob/main/README.md) (no duplicar aquí)                                                                                                                    |
| Program IDs, seeds, CPI, lifecycle   | Este directorio (`docs/`)                                                                                                                                                                                         |
| TODOs de seguridad en código         | [stayke-todos-security.guide.md](./stayke-todos-security.guide.md)                                                                                                                                                |




## Índice de guías


| Guía                                                                     | Contenido                                            |
| ------------------------------------------------------------------------ | ---------------------------------------------------- |
| [stayke-architecture-flow.guide.md](./stayke-architecture-flow.guide.md) | Vista corta: roles + CPI (sin dump de System Design) |
| [stayke-global.guide.md](./stayke-global.guide.md)                       | Referencia técnica: IDs, cuentas, mapa CPI           |
| [stayke-flow-diagram.md](./stayke-flow-diagram.md)                       | Diagramas Mermaid alineados al código                |
| [stayke-core.guide.md](./stayke-core.guide.md)                           | Identidad, perfiles, listings                        |
| [stayke-escrow.guide.md](./stayke-escrow.guide.md)                       | Booking + gate `minimum_deposit`                     |
| [stayke-disputes.guide.md](./stayke-disputes.guide.md)                   | Disputas; `resolve` ≠ `penalize`                     |
| [stayke-treasury.guide.md](./stayke-treasury.guide.md)                   | Garantías + `cpi_penalize_transfer`                  |
| [stayke-config.guide.md](./stayke-config.guide.md)                       | `GlobalConfig` (incompleto vs objetivo CPI)          |
| [stayke-todos-security.guide.md](./stayke-todos-security.guide.md)       | TODOs / riesgos de seguridad                         |




## Checklist de lectura

- [ ] Sé distinguir SoT vs as-implemented
- [ ] Abrí la guía del programa que estoy integrando
- [ ] Si veo un callout de gap, no lo trato como política ya cumplida on-chain