# 星河科技 2026 年度经营复盘

本文档用于试用数据图表页：覆盖五种图表类型、图文同页、长文拆页、一节多图、章节级图表等情况。

## 年度业绩概览

全年营收 1520 万元，同比增长 38%。四个季度逐季走高，第四季度受年末集中交付拉动，单季营收首次突破 500 万元，是全年最重要的增长来源。

```chart
{
  "type": "bar",
  "title": "季度营收（万元）",
  "data": [
    { "quarter": "Q1", "revenue": 320 },
    { "quarter": "Q2", "revenue": 260 },
    { "quarter": "Q3", "revenue": 410 },
    { "quarter": "Q4", "revenue": 530 }
  ],
  "encoding": { "x": "quarter", "y": "revenue" },
  "emphasis": { "highlight": ["Q4"] },
  "source": "user_input",
  "source_note": "财务部年度报表"
}
```

### 收入结构

渠道合作仍是最大的收入来源，占比接近一半；官网与搜索带来的自然流量稳定在三成左右；线下活动投入不小但转化偏低。

```chart
{
  "type": "donut",
  "title": "收入来源构成（万元）",
  "data": [
    { "channel": "渠道合作", "amount": 700 },
    { "channel": "官网直销", "amount": 270 },
    { "channel": "搜索投放", "amount": 200 },
    { "channel": "老客推荐", "amount": 210 },
    { "channel": "线下活动", "amount": 140 }
  ],
  "encoding": { "label": "channel", "value": "amount" },
  "emphasis": { "highlight": ["渠道合作"] },
  "source": "user_input"
}
```

### 增长趋势

月度交付量全年保持上升，没有出现回落。上半年增长主要依靠存量客户的扩容需求，月均新增交付在三个左右，节奏平稳但斜率不高。五月份自动化交付流水线正式上线，需求从评审到上线的平均周期由二十一天缩短到九天，当月交付量环比跳升三成，此后一直维持在较高水平。下半年新签的三个行业大客户陆续进入实施期，十月和十一月出现了全年的两个交付高峰，团队一度通过跨组借调人员来保证进度。十二月因为年末封网，交付量略有放缓，但全年累计交付仍超过年初目标两成。需要注意的是，交付量的快速增长也带来了运维压力，线上问题单数量同步上升，明年需要在质量保障上补课，把自动化测试覆盖率从目前的四成提升到七成以上，并建立按客户分级的值守机制，避免高峰期响应变慢。从交付结构看，标准化模块的复用比例由年初的三成提升到年末的六成，这是交付效率提升的根本原因；而定制化开发仍集中在少数大客户身上，后续应通过产品化沉淀把这部分工作逐步转化为可复用的能力，进一步降低单个项目的交付成本。

```chart
{
  "type": "line",
  "title": "月度交付量（个）",
  "data": [
    { "month": "1月", "count": 18 },
    { "month": "2月", "count": 17 },
    { "month": "3月", "count": 21 },
    { "month": "4月", "count": 24 },
    { "month": "5月", "count": 31 },
    { "month": "6月", "count": 33 },
    { "month": "7月", "count": 34 },
    { "month": "8月", "count": 36 },
    { "month": "9月", "count": 38 },
    { "month": "10月", "count": 45 },
    { "month": "11月", "count": 47 },
    { "month": "12月", "count": 41 }
  ],
  "encoding": { "x": "month", "y": "count" },
  "emphasis": { "highlight": ["5月", "11月"] },
  "source": "user_input",
  "source_note": "交付平台月报"
}
```

## 产品线表现

```chart
{
  "type": "grouped_bar",
  "title": "三条产品线季度签约额（万元）",
  "data": [
    { "quarter": "Q1", "a": 120, "b": 80, "c": 40 },
    { "quarter": "Q2", "a": 135, "b": 82, "c": 55 },
    { "quarter": "Q3", "a": 150, "b": 90, "c": 88 },
    { "quarter": "Q4", "a": 170, "b": 95, "c": 120 }
  ],
  "encoding": { "x": "quarter" },
  "series": [
    { "key": "a", "name": "数据中台" },
    { "key": "b", "name": "智能客服" },
    { "key": "c", "name": "知识库" }
  ],
  "source": "user_input"
}
```

### 数据中台

数据中台是公司的基本盘，全年签约额 575 万元，客户续约率 92%。下半年开始从单点项目转向区域整体方案，客单价明显提高。

```chart
{
  "type": "line",
  "title": "数据中台平均客单价（万元）",
  "data": [
    { "quarter": "Q1", "price": 38 },
    { "quarter": "Q2", "price": 41 },
    { "quarter": "Q3", "price": 55 },
    { "quarter": "Q4", "price": 63 }
  ],
  "encoding": { "x": "quarter", "y": "price" },
  "source": "user_input"
}
```

```chart
{
  "type": "bar",
  "title": "数据中台客户续约率（%）",
  "data": [
    { "year": "2023", "rate": 81 },
    { "year": "2024", "rate": 86 },
    { "year": "2025", "rate": 89 },
    { "year": "2026", "rate": 92 }
  ],
  "encoding": { "x": "year", "y": "rate" },
  "emphasis": { "highlight": ["2026"] },
  "source": "user_input"
}
```

### 知识库与智能客服对比

知识库产品增速最快，下半年签约额翻倍，但单客规模小、交付标准化程度高；智能客服增长平稳，单客规模大，但定制开发多、交付周期长。两者在目标客户上高度重叠，适合打包销售。

## 成本与效率

### 算力成本

推理成本占比持续扩大，已经超过训练成本，成为算力支出的主体；存储成本增长缓慢，整体可控。

```chart
{
  "type": "stacked_area",
  "title": "月度算力成本（万元）",
  "data": [
    { "month": "7月", "train": 40, "infer": 25, "storage": 10 },
    { "month": "8月", "train": 42, "infer": 32, "storage": 11 },
    { "month": "9月", "train": 41, "infer": 40, "storage": 11 },
    { "month": "10月", "train": 43, "infer": 51, "storage": 12 },
    { "month": "11月", "train": 42, "infer": 63, "storage": 13 },
    { "month": "12月", "train": 44, "infer": 70, "storage": 13 }
  ],
  "encoding": { "x": "month" },
  "series": [
    { "key": "train", "name": "训练" },
    { "key": "infer", "name": "推理" },
    { "key": "storage", "name": "存储" }
  ],
  "source": "llm_generated"
}
```

### 主要问题与风险

一是交付高峰期人手不足，跨组借调影响了其他项目进度；二是线上问题单数量随交付量同步上升，质量保障能力跟不上；三是推理成本增长过快，部分低价合同已经出现亏损风险。

## 明年计划

第一阶段（一季度）完成自动化测试体系建设，覆盖率提升到七成；第二阶段（二季度）上线推理成本监控与按量计费；第三阶段（下半年）推出知识库与智能客服的打包方案，并在三个重点区域复制数据中台整体方案。
