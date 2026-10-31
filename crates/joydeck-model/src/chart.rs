//! Independent bounded chart data and renderer. Numeric cache and workbook share this model.
use crate::document::{Object, Result, color};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    pub name: String,
    pub values: Vec<Option<f64>>,
    #[serde(default)]
    pub x: Vec<Option<f64>>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Chart {
    pub kind: String,
    pub title: String,
    pub categories: Vec<String>,
    pub series: Vec<Series>,
    pub legend: bool,
    pub grid: bool,
    pub labels: bool,
    pub x_title: String,
    pub y_title: String,
    pub unit: String,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub colors: Vec<String>,
}
impl Default for Chart {
    fn default() -> Self {
        Self {
            kind: "column".into(),
            title: "季度业务表现".into(),
            categories: vec![
                "第一季度".into(),
                "第二季度".into(),
                "第三季度".into(),
                "第四季度".into(),
            ],
            series: vec![Series {
                name: "收入".into(),
                values: vec![Some(42.), Some(58.), Some(72.), Some(91.)],
                x: vec![],
            }],
            legend: true,
            grid: true,
            labels: false,
            x_title: String::new(),
            y_title: String::new(),
            unit: String::new(),
            minimum: None,
            maximum: None,
            colors: vec![
                "#4472c4".into(),
                "#ed7d31".into(),
                "#a5a5a5".into(),
                "#ffc000".into(),
                "#5b9bd5".into(),
                "#70ad47".into(),
            ],
        }
    }
}
impl Chart {
    pub fn validate(&self) -> Result<()> {
        if ![
            "column", "bar", "line", "area", "pie", "doughnut", "scatter",
        ]
        .contains(&self.kind.as_str())
            || self.categories.is_empty()
            || self.categories.len() > 256
            || self.series.is_empty()
            || self.series.len() > 16
            || self.categories.len() * self.series.len() > 4096
            || self.colors.is_empty()
            || self.colors.len() > 32
            || self
                .colors
                .iter()
                .any(|c| !color(c) && !matches!(c.as_str(), "@accent" | "@background"))
        {
            return Err("图表类型、维度或颜色超出支持范围".into());
        }
        if self
            .categories
            .iter()
            .chain([&self.title, &self.x_title, &self.y_title, &self.unit])
            .any(|s| {
                s.len() > 2048
                    || s.chars()
                        .any(|c| c < ' ' && !matches!(c, '\n' | '\r' | '\t'))
            })
        {
            return Err("图表标签超限".into());
        }
        if self.unit.contains(['"', '\\', '\n', '\r']) {
            return Err("单位不能包含引号、反斜杠或换行".into());
        }
        for s in &self.series {
            if s.name.chars().any(|c| c < ' ')
                || s.name.contains(['\t', '\n', '\r'])
                || self
                    .categories
                    .iter()
                    .any(|c| c.contains(['\t', '\n', '\r']))
                || s.name.len() > 256
                || s.values.len() != self.categories.len()
                || (!s.x.is_empty() && s.x.len() != s.values.len())
                || s.values
                    .iter()
                    .chain(&s.x)
                    .flatten()
                    .any(|v| !v.is_finite() || v.abs() > 1e12)
            {
                return Err("图表系列长度/数值无效".into());
            }
        }
        if self
            .minimum
            .into_iter()
            .chain(self.maximum)
            .any(|v| !v.is_finite() || v.abs() > 1e12)
            || self.minimum.zip(self.maximum).is_some_and(|(a, b)| a >= b)
        {
            return Err("图表坐标轴范围无效".into());
        }
        if matches!(self.kind.as_str(), "pie" | "doughnut")
            && (self.series.len() != 1
                || self.series[0].values.iter().flatten().any(|v| *v < 0.)
                || self.series[0].values.iter().flatten().sum::<f64>() <= 0.)
        {
            return Err("饼图需要一个非负且总和大于零的系列".into());
        }
        Ok(())
    }
    pub fn tsv(&self) -> String {
        if self.kind == "scatter" {
            let mut rows = vec![
                self.series
                    .iter()
                    .flat_map(|s| [format!("{} X", s.name), s.name.clone()])
                    .collect::<Vec<_>>()
                    .join("\t"),
            ];
            for i in 0..self.categories.len() {
                rows.push(
                    self.series
                        .iter()
                        .flat_map(|s| {
                            [
                                if s.x.is_empty() {
                                    Some((i + 1) as f64)
                                } else {
                                    s.x[i]
                                },
                                s.values[i],
                            ]
                        })
                        .map(|v| v.map_or(String::new(), |v| v.to_string()))
                        .collect::<Vec<_>>()
                        .join("\t"),
                );
            }
            return rows.join("\n");
        }
        let mut lines = vec![
            std::iter::once("类别".to_owned())
                .chain(self.series.iter().map(|s| s.name.clone()))
                .collect::<Vec<_>>()
                .join("\t"),
        ];
        for (i, c) in self.categories.iter().enumerate() {
            lines.push(
                std::iter::once(c.clone())
                    .chain(
                        self.series
                            .iter()
                            .map(|s| s.values[i].map_or(String::new(), |n| n.to_string())),
                    )
                    .collect::<Vec<_>>()
                    .join("\t"),
            );
        }
        lines.join("\n")
    }
    pub fn paste(&mut self, value: &str) -> Result<()> {
        if value.len() > 131072 {
            return Err("图表数据超过128KiB预算".into());
        }
        let text = value.replace("\r\n", "\n").replace('\r', "\n");
        let rows: Vec<Vec<_>> = text
            .trim_end_matches('\n')
            .split('\n')
            .map(|r| r.split('\t').collect())
            .collect();
        if rows.len() < 2
            || rows.len() > 257
            || rows[0].len() < 2
            || rows[0].len() > if self.kind == "scatter" { 32 } else { 17 }
            || rows.iter().any(|r| r.len() != rows[0].len())
        {
            return Err("图表TSV需要首行系列名称和有界矩形数据".into());
        }
        let mut d = self.clone();
        if self.kind == "scatter" {
            if rows[0].len() % 2 != 0 {
                return Err("散点图需要 X/Y 成对的列".into());
            }
            d.categories = (1..rows.len()).map(|i| i.to_string()).collect();
            d.series = (0..rows[0].len())
                .step_by(2)
                .map(|col| {
                    let parse = |c: usize| {
                        rows[1..]
                            .iter()
                            .map(|r| {
                                if r[c].trim().is_empty() {
                                    Ok(None)
                                } else {
                                    r[c].trim()
                                        .parse::<f64>()
                                        .map(Some)
                                        .map_err(|_| "散点值无效".to_owned())
                                }
                            })
                            .collect::<Result<Vec<_>>>()
                    };
                    Ok(Series {
                        name: rows[0][col + 1].into(),
                        x: parse(col)?,
                        values: parse(col + 1)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            d.validate()?;
            *self = d;
            return Ok(());
        }
        d.categories = rows[1..].iter().map(|r| r[0].into()).collect();
        d.series = (1..rows[0].len())
            .map(|c| {
                let values = rows[1..]
                    .iter()
                    .map(|r| {
                        if r[c].trim().is_empty() {
                            Ok(None)
                        } else {
                            r[c].trim()
                                .parse::<f64>()
                                .map(Some)
                                .map_err(|_| "图表数据须为数值或空单元格".to_owned())
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Series {
                    name: rows[0][c].into(),
                    values,
                    x: vec![],
                })
            })
            .collect::<Result<Vec<_>>>()?;
        d.validate()?;
        *self = d;
        Ok(())
    }
    pub fn domain(&self) -> (f64, f64) {
        let mut lo = 0f64;
        let mut hi = 0f64;
        for n in self.series.iter().flat_map(|s| s.values.iter()).flatten() {
            lo = lo.min(*n);
            hi = hi.max(*n);
        }
        lo = self.minimum.unwrap_or(lo);
        hi = self.maximum.unwrap_or(hi);
        if lo >= hi {
            if self.maximum.is_some() && self.minimum.is_none() {
                lo = hi - 1.;
            } else {
                hi = lo + 1.;
            }
        }
        (lo, hi)
    }
}
pub fn object(kind: &str) -> Object {
    let mut o = crate::identity::object("chart");
    let c = Chart {
        kind: kind.into(),
        ..Default::default()
    };
    o.chart = Some(c);
    o.x = 100.;
    o.y = 90.;
    o.width = 700.;
    o.height = 360.;
    o
}
