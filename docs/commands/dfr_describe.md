---
title: dfr describe
layout: command
version: 0.59.0
---

Describes dataframes numeric columns

## Signature

```> dfr describe --quantiles```

## Parameters

 -  `--quantiles {table}`: optional quantiles for describe

## Examples

dataframe description
```shell
> [[a b]; [1 1] [1 1]] | dfr to-df | dfr describe
```
