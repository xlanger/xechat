"""XEChat LanceDB 开发调试工具。

纯只读调试工具，基于 Pandas + LanceDB 原生 API，
与应用数据目录直接联动，实时查看对话、记忆轮次、向量索引等关键指标。

XEChat LanceDB 包含两个表：
- conversations: 对话+消息（每条消息一行）
- turns: 记忆轮次+向量（语义分块后每块一行，含 768 维 E5 向量）

用法:
    from lance_debug import LanceDebugger
    db = LanceDebugger()
    db.info()
    db.conversations.list()
    db.turns.list()
"""

from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import Optional

import lancedb
import pandas as pd


def _default_lancedb_path() -> Path:
    """自动发现 LanceDB 数据目录（跨平台标准路径）。"""
    # 1. 环境变量
    env_path = os.environ.get("LANCE_DB_PATH")
    if env_path:
        return Path(env_path)

    # 2. 按平台标准路径查找
    system = sys.platform

    if system == "darwin":
        # macOS: ~/Library/Application Support/xechat/lancedb/
        macos_path = Path.home() / "Library" / "Application Support" / "xechat" / "lancedb"
        if macos_path.exists():
            return macos_path
    elif system == "win32":
        # Windows: %LOCALAPPDATA%\xechat\lancedb\
        local_app = os.environ.get("LOCALAPPDATA")
        if local_app:
            win_path = Path(local_app) / "xechat" / "lancedb"
            if win_path.exists():
                return win_path
    elif system.startswith("linux"):
        # Linux: ~/.local/share/xechat/lancedb/
        linux_path = Path.home() / ".local" / "share" / "xechat" / "lancedb"
        if linux_path.exists():
            return linux_path

    # 3. 回退：依次尝试所有平台路径
    candidates = []
    if system == "darwin":
        candidates.append(Path.home() / "Library" / "Application Support" / "xechat" / "lancedb")
    elif system == "win32":
        local_app = os.environ.get("LOCALAPPDATA", "")
        if local_app:
            candidates.append(Path(local_app) / "xechat" / "lancedb")
    elif system.startswith("linux"):
        candidates.append(Path.home() / ".local" / "share" / "xechat" / "lancedb")

    # 旧版路径兼容
    candidates.append(Path.home() / ".xechat" / "lancedb")

    for p in candidates:
        if p.exists():
            return p

    # 最终回退到当前平台默认（即使不存在，让后续报错更清晰）
    if system == "darwin":
        return Path.home() / "Library" / "Application Support" / "xechat" / "lancedb"
    elif system == "win32":
        local_app = os.environ.get("LOCALAPPDATA", str(Path.home() / "AppData" / "Local"))
        return Path(local_app) / "xechat" / "lancedb"
    else:
        return Path.home() / ".local" / "share" / "xechat" / "lancedb"


class TableInspector:
    """LanceDB 表级只读检查器。"""

    def __init__(self, db: lancedb.DBConnection, table_name: str):
        self._db = db
        self._table_name = table_name
        self._table = db.open_table(table_name)

    @property
    def table_name(self) -> str:
        return self._table_name

    def info(self) -> None:
        """打印表详情：行数、schema、索引。"""
        schema = self._table.schema
        row_count = self._table.count_rows()
        print(f"表: {self._table_name}")
        print(f"行数: {row_count}")
        print(f"Schema:")
        for field in schema:
            nullable = "nullable" if field.nullable else "required"
            print(f"  {field.name}: {field.type} ({nullable})")

    def list(self, limit: int = 20) -> pd.DataFrame:
        """返回对话/轮次摘要列表。"""
        df = self._table.to_pandas()
        if self._table_name == "conversations":
            # 按 conversation_id 分组，取最新 updated_at 和消息数
            result = (
                df.groupby("conversation_id")
                .agg(
                    title=("title", "first"),
                    updated_at=("updated_at", "max"),
                    message_count=("message_id", "count"),
                )
                .reset_index()
                .sort_values("updated_at", ascending=False)
                .head(limit)
            )
            return result
        elif self._table_name == "turns":
            # 按 conversation_id 分组，取轮次数
            result = (
                df.groupby("conversation_id")
                .agg(
                    turn_count=("id", "count"),
                    has_vector=("vector", lambda x: x.notna().sum()),
                    latest_timestamp=("timestamp", "max"),
                )
                .reset_index()
                .sort_values("latest_timestamp", ascending=False)
                .head(limit)
            )
            return result
        else:
            return df.head(limit)

    def get(self, conv_id: str) -> pd.DataFrame:
        """获取指定对话的所有消息（conversations 表）。"""
        df = self._table.search().where(f"conversation_id = '{conv_id}'").to_pandas()
        return df.sort_values("timestamp", ascending=True)

    def get_turns(self, conv_id: str) -> pd.DataFrame:
        """获取指定对话的所有记忆轮次（turns 表）。"""
        df = self._table.search().where(f"conversation_id = '{conv_id}'").to_pandas()
        return df.sort_values("turn_index", ascending=True)

    def search(self, query: str, limit: int = 10) -> pd.DataFrame:
        """全文搜索。"""
        try:
            search_col = "chunk_text" if self._table_name == "turns" else "content"
            return self._table.search(query, query_type="fts").select([]).limit(limit).to_pandas()
        except Exception as e:
            print(f"[降级] 全文搜索失败 ({e})，使用 SQL LIKE 查询")
            return self._like_search(query, limit)

    def _like_search(self, query: str, limit: int = 10) -> pd.DataFrame:
        """SQL LIKE 模糊搜索（倒排索引不可用时的降级方案）。"""
        escaped = query.replace("'", "''")
        df = self._table.to_pandas()
        search_cols = []
        if self._table_name == "conversations":
            search_cols = [c for c in ["content", "title"] if c in df.columns]
        elif self._table_name == "turns":
            search_cols = [c for c in ["chunk_text", "user_content", "assistant_content"] if c in df.columns]
        if not search_cols:
            return df.head(limit)
        mask = df[search_cols[0]].str.contains(escaped, case=False, na=False)
        for col in search_cols[1:]:
            mask |= df[col].str.contains(escaped, case=False, na=False)
        return df[mask].head(limit)

    def create_fts_index(self, columns: list[str] | None = None) -> None:
        """创建全文搜索倒排索引（写操作，仅在需要时手动调用）。

        Args:
            columns: 要索引的列名列表。
                     conversations 表默认 ["content"]，
                     turns 表默认 ["chunk_text"]
        """
        if columns is None:
            columns = ["chunk_text"] if self._table_name == "turns" else ["content"]
        print(f"正在创建倒排索引: {columns} ...")
        for col in columns:
            self._table.create_fts_index(col, replace=True, use_tantivy=False)
        print("倒排索引创建完成")

    def create_vector_index(self, column: str = "vector", metric: str = "cosine", min_rows: int = 256) -> None:
        """创建向量索引 IVF_PQ（写操作，仅在需要时手动调用）。

        仅适用于 turns 表（含 vector 列）。

        Args:
            column: 向量列名，默认为 "vector"
            metric: 距离度量，默认为 "cosine"
            min_rows: 最少行数阈值，低于此值跳过
        """
        if self._table_name != "turns":
            print("向量索引仅适用于 turns 表")
            return
        row_count = self._table.count_rows()
        if row_count < min_rows:
            print(f"跳过向量索引创建：当前 {row_count} 行，IVF_PQ 最少需要 {min_rows} 行")
            return
        print(f"正在创建向量索引 (IVF_PQ): {column}, metric={metric} ({row_count} rows) ...")
        self._table.create_index(metric=metric, vector_column_name=column, index_type="IVF_PQ", replace=True)
        print("向量索引创建完成")

    def create_all_indexes(self) -> None:
        """一键补建所有索引。"""
        if self._table_name == "conversations":
            self.create_fts_index(["content"])
        elif self._table_name == "turns":
            self.create_fts_index(["chunk_text"])
            self.create_vector_index("vector")

    def vector_stats(self) -> dict:
        """向量统计：覆盖率、维度、空值数（仅 turns 表）。"""
        if self._table_name != "turns":
            print("向量统计仅适用于 turns 表")
            return {}
        df = self._table.to_pandas()
        total = len(df)
        if "vector" not in df.columns:
            return {
                "total_rows": total,
                "rows_with_vector": 0,
                "rows_without_vector": total,
                "coverage_pct": 0.0,
                "vector_dim": 0,
            }

        has_vector = df["vector"].notna()
        rows_with = int(has_vector.sum())
        rows_without = total - rows_with
        coverage = (rows_with / total * 100) if total > 0 else 0.0

        # 采样计算 L2 范数
        import numpy as np
        sample_vecs = df["vector"].dropna().head(10)
        norms = []
        for v in sample_vecs:
            arr = np.array(v)
            norms.append(float(np.linalg.norm(arr)))

        dim = len(sample_vecs.iloc[0]) if len(sample_vecs) > 0 else 0

        return {
            "total_rows": total,
            "rows_with_vector": rows_with,
            "rows_without_vector": rows_without,
            "coverage_pct": round(coverage, 1),
            "vector_dim": dim,
            "sample_norms": [round(n, 4) for n in norms],
        }

    def vector_sample(self, n: int = 5) -> pd.DataFrame:
        """随机采样向量行（仅 turns 表）。"""
        if self._table_name != "turns":
            print("向量采样仅适用于 turns 表")
            return pd.DataFrame()
        df = self._table.to_pandas()
        if "vector" in df.columns:
            df = df[df["vector"].notna()]
        # 排除 vector 列（太大），只展示元数据
        meta_cols = [c for c in df.columns if c != "vector"]
        return df[meta_cols].sample(n=min(n, len(df)), random_state=42)

    def vector_pca_plot(self, n: int = 200, color_by: str = "conversation_id"):
        """PCA 降维 2D 散点图（仅 turns 表）。"""
        if self._table_name != "turns":
            print("PCA 图仅适用于 turns 表")
            return

        import numpy as np
        from sklearn.decomposition import PCA
        import matplotlib
        import matplotlib.pyplot as plt

        matplotlib.use("Agg") if not self._is_notebook() else matplotlib.use("MacOSX")

        df = self._table.to_pandas()
        if "vector" not in df.columns:
            print("无向量列，无法绘制 PCA 图")
            return

        df = df[df["vector"].notna()]
        if len(df) == 0:
            print("无向量数据，无法绘制 PCA 图")
            return

        sample = df.sample(n=min(n, len(df)), random_state=42)
        vectors = np.stack(sample["vector"].values)
        labels = sample[color_by].values if color_by in sample.columns else None

        pca = PCA(n_components=2)
        coords = pca.fit_transform(vectors)

        fig, ax = plt.subplots(figsize=(10, 7))
        if labels is not None:
            unique_labels = list(set(labels))
            colors = plt.cm.tab20(np.linspace(0, 1, len(unique_labels)))
            label_to_color = {l: c for l, c in zip(unique_labels, colors)}
            for label in unique_labels:
                mask = labels == label
                short = label[:8]
                ax.scatter(coords[mask, 0], coords[mask, 1], c=[label_to_color[label]], label=short, alpha=0.6, s=20)
            ax.legend(bbox_to_anchor=(1.05, 1), loc="upper left", fontsize=8)
        else:
            ax.scatter(coords[:, 0], coords[:, 1], alpha=0.6, s=20)

        ax.set_title(f"PCA of {self._table_name} vectors (n={len(sample)})")
        ax.set_xlabel(f"PC1 ({pca.explained_variance_ratio_[0]:.1%})")
        ax.set_ylabel(f"PC2 ({pca.explained_variance_ratio_[1]:.1%})")
        plt.tight_layout()

        if self._is_notebook():
            plt.show()
        else:
            plt.savefig("lance_pca.png", dpi=150)
            print("PCA 图已保存到 lance_pca.png")

    def time_range(self) -> tuple[str, str]:
        """最早/最晚时间。"""
        df = self._table.to_pandas()
        ts_col = "timestamp" if "timestamp" in df.columns else "updated_at"
        earliest = df[ts_col].min()
        latest = df[ts_col].max()
        print(f"时间范围: {earliest} ~ {latest}")
        return (str(earliest), str(latest))

    def time_distribution(self, freq: str = "D") -> pd.DataFrame:
        """按天/周/月的消息分布。"""
        import matplotlib
        import matplotlib.pyplot as plt

        matplotlib.use("Agg") if not self._is_notebook() else matplotlib.use("MacOSX")

        df = self._table.to_pandas()
        ts_col = "timestamp" if "timestamp" in df.columns else "updated_at"
        df[ts_col] = pd.to_datetime(df[ts_col])
        df["time_bucket"] = df[ts_col].dt.floor(freq)
        dist = df.groupby("time_bucket").size().reset_index(name="count")

        fig, ax = plt.subplots(figsize=(12, 4))
        ax.bar(dist["time_bucket"], dist["count"], width=0.8)
        label = "消息" if self._table_name == "conversations" else "轮次"
        ax.set_title(f"{self._table_name} {label}时间分布 (freq={freq})")
        ax.set_xlabel("时间")
        ax.set_ylabel(f"{label}数")
        plt.xticks(rotation=45)
        plt.tight_layout()

        if self._is_notebook():
            plt.show()
        else:
            plt.savefig("lance_time_dist.png", dpi=150)
            print("时间分布图已保存到 lance_time_dist.png")

        return dist

    @staticmethod
    def _is_notebook() -> bool:
        try:
            get_ipython()  # type: ignore[name-defined]
            return True
        except NameError:
            return False


class LanceDebugger:
    """XEChat LanceDB 开发调试工具入口。"""

    def __init__(self, path: Optional[str] = None):
        if path is None:
            db_path = _default_lancedb_path()
        else:
            db_path = Path(path)

        if not db_path.exists():
            raise FileNotFoundError(
                f"LanceDB 数据目录不存在: {db_path}\n"
                f"请确认 XEChat 已运行过，或设置 LANCE_DB_PATH 环境变量"
            )

        self._path = db_path
        self._db = lancedb.connect(str(db_path))

        # 打开存在的表
        self._tables: dict[str, TableInspector] = {}
        for name in self._db.table_names():
            self._tables[name] = TableInspector(self._db, name)

    @property
    def conversations(self) -> TableInspector:
        """conversations 表：对话+消息（每条消息一行）。

        Schema:
            conversation_id: string (not null)
            title: string (not null)
            created_at: string (RFC3339, not null)
            updated_at: string (RFC3339, not null)
            message_id: string (not null)
            role: string ("User" | "Assistant", not null)
            content: string (not null)
            reasoning_content: string (not null, 可为空字符串)
            status: string ("Sending" | "Sent" | "Failed" | "Truncated", not null)
            timestamp: string (RFC3339, not null)
        """
        if "conversations" not in self._tables:
            raise KeyError("conversations 表不存在")
        return self._tables["conversations"]

    @property
    def turns(self) -> TableInspector:
        """turns 表：记忆轮次+向量（语义分块后每块一行）。

        Schema:
            id: string (not null)
            conversation_id: string (not null)
            user_message_id: string (not null)
            assistant_message_id: string (not null)
            turn_index: int32 (not null)
            user_content: string (not null)
            assistant_content: string (not null)
            chunk_index: int32 (not null)
            chunk_text: string (not null, FTS 索引)
            start_char: int32 (not null)
            end_char: int32 (not null)
            timestamp: string (RFC3339, not null)
            vector: fixed_size_list<float32>[768] (nullable, E5 嵌入)
        """
        if "turns" not in self._tables:
            raise KeyError("turns 表不存在")
        return self._tables["turns"]

    def info(self) -> None:
        """打印概览：表列表、行数、数据目录。"""
        print(f"LanceDB 数据目录: {self._path}")
        print(f"表列表: {list(self._tables.keys())}")
        print()
        for name, inspector in self._tables.items():
            print(f"--- {name} ---")
            inspector.info()
            print()

    def close(self) -> None:
        """关闭连接。"""
        pass  # lancedb Python 不需要显式关闭
