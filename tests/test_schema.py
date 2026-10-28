"""Stage 1 database contract tests; uses only Python's standard library.

These exercise the actual versioned SQL, not a reimplementation of Rust services.
Run: python3 -m unittest discover -s tests -v
"""
from pathlib import Path
from datetime import date, timedelta
from contextlib import closing
import hashlib
import sqlite3
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
MIGRATIONS = ROOT / "src-tauri" / "migrations"


def execute_sql(connection, source):
    """Unlike executescript, preserve the caller's transaction."""
    buffer = ""
    for line in source.splitlines(keepends=True):
        buffer += line
        if sqlite3.complete_statement(buffer):
            connection.execute(buffer)
            buffer = ""
    if buffer.strip():
        connection.execute(buffer)


def migrate(connection):
    connection.execute("PRAGMA foreign_keys = ON")
    connection.execute("PRAGMA recursive_triggers = ON")
    for path in sorted(MIGRATIONS.glob("*.sql")):
        source = path.read_text(encoding="utf-8")
        connection.execute("BEGIN IMMEDIATE")
        try:
            execute_sql(connection, source)
            connection.execute(
                "INSERT INTO schema_migrations(version, name, sha256) VALUES (?, ?, ?)",
                (int(path.name.split("_")[0]), path.name, hashlib.sha256(source.encode()).hexdigest()),
            )
            if connection.execute("PRAGMA foreign_key_check").fetchall():
                raise AssertionError("Migration introduced invalid foreign keys")
            connection.execute("COMMIT")
        except Exception:
            connection.execute("ROLLBACK")
            raise


class SchemaTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.template = sqlite3.connect(":memory:", isolation_level=None)
        migrate(cls.template)

    @classmethod
    def tearDownClass(cls):
        cls.template.close()

    def setUp(self):
        self.db = sqlite3.connect(":memory:", isolation_level=None)
        self.template.backup(self.db)
        self.db.execute("PRAGMA foreign_keys = ON")
        self.db.execute("PRAGMA recursive_triggers = ON")
        # Synthetic hash marker is never used for login or shipped as reference data.
        self.db.execute("INSERT INTO users(id, username, display_name, password_hash) VALUES (1, 'tester', 'اختبار', '$argon2id$test-only')")
        self.db.execute("INSERT INTO fiscal_periods(id, name, starts_on, ends_on) VALUES (1, 'اختبار', '2026-01-01', '2026-12-31')")
        self.db.execute("INSERT INTO parties(id, code, name_ar, normalized_name, is_customer, is_supplier) VALUES (1, 'P1', 'طرف اختبار', 'طرف اختبار', 1, 1)")
        self.db.execute("INSERT INTO products(id, code, name_ar, normalized_name, category_id, base_unit_id) VALUES (1, 'PPR20', 'ماسورة مياه', 'ماسوره مياه', 1, 1)")
        self.db.execute("INSERT INTO product_units(id, product_id, unit_id, factor_numerator, barcode, retail_price_minor, wholesale_price_minor) VALUES (1, 1, 1, 1, '6220000000001', 1500, 1400)")

    def tearDown(self):
        self.db.close()

    def doc(self, kind, total=0, **fields):
        values = dict(kind=kind, document_date="2026-10-27", party_id=1,
                      warehouse_id=1, currency_code="EGP", currency_symbol="ج.م",
                      currency_minor_digits=2, subtotal_minor=total, total_minor=total,
                      created_by=1)
        values.update(fields)
        columns = ", ".join(values)
        placeholders = ", ".join("?" for _ in values)
        doc_id = self.db.execute(f"INSERT INTO documents({columns}) VALUES ({placeholders})", tuple(values.values())).lastrowid
        self.db.execute("UPDATE documents SET number = ? WHERE id = ?", (f"TEST-{doc_id}", doc_id))
        return doc_id

    def line(self, doc, quantity=1000000, amount=1500, **fields):
        values = dict(document_id=doc, line_number=1, product_id=1, product_unit_id=1,
                      product_code_snapshot="PPR20", product_name_snapshot="ماسورة مياه",
                      unit_name_snapshot="قطعة", category_id_snapshot=1,
                      factor_numerator=1, factor_denominator=1, quantity_micros=quantity,
                      base_quantity_micros=quantity, unit_price_minor=1500,
                      subtotal_minor=amount, total_minor=amount)
        values.update(fields)
        return self.db.execute(
            f"INSERT INTO document_lines({', '.join(values)}) VALUES ({', '.join('?' for _ in values)})",
            tuple(values.values()),
        ).lastrowid

    def start(self, doc):
        self.db.execute("UPDATE documents SET status = 'posting' WHERE id = ?", (doc,))

    def finish(self, doc):
        self.db.execute("UPDATE documents SET status = 'posted', posted_by = 1, posted_at = '2026-10-27T10:00:00Z' WHERE id = ?", (doc,))

    def stock(self, doc, line, quantity, value, kind, warehouse=1, reverse=None):
        return self.db.execute("""INSERT INTO stock_movements(document_id, document_line_id, product_id,
            warehouse_id, movement_type, quantity_micros, value_minor, unit_cost_micropiastres,
            created_by, reverses_movement_id) VALUES (?, ?, 1, ?, ?, ?, ?, 1000000000, 1, ?)""",
            (doc, line, warehouse, kind, quantity, value, reverse)).lastrowid

    def journal(self, doc, entries):
        entry = self.db.execute("INSERT INTO journal_entries(document_id) VALUES (?)", (doc,)).lastrowid
        line_ids = []
        for index, (account, debit, credit) in enumerate(entries, 1):
            party = 1 if account in (110, 201) else None
            due = "2026-11-27" if party else None
            line_ids.append(self.db.execute("""INSERT INTO journal_lines(entry_id, document_id,
                line_number, account_id, debit_minor, credit_minor, party_id, due_date)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)""", (entry, doc, index, account, debit, credit, party, due)).lastrowid)
        return line_ids

    def opening(self, quantity=10000000, value=10000):
        doc = self.doc("opening_stock", value)
        line = self.line(doc, quantity, value)
        self.start(doc)
        self.stock(doc, line, quantity, value, "opening")
        self.journal(doc, [(120, value, 0), (303, 0, value)])
        self.finish(doc)
        return doc

    def sale(self, quantity=2000000, amount=3000, cost=2000, credit=False):
        doc = self.doc("sale", amount)
        line = self.line(doc, quantity, amount)
        self.start(doc)
        self.stock(doc, line, -quantity, -cost, "sale")
        journal = self.journal(doc, [(110 if credit else 101, amount, 0), (401, 0, amount), (501, cost, 0), (120, 0, cost)])
        self.finish(doc)
        return doc, line, journal[0]

    def test_schema_integrity_and_reference_data(self):
        self.assertEqual(self.db.execute("PRAGMA integrity_check").fetchall(), [("ok",)])
        self.assertEqual(self.db.execute("PRAGMA foreign_key_check").fetchall(), [])
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM schema_migrations").fetchone()[0], len(list(MIGRATIONS.glob("*.sql"))))
        self.assertEqual(self.template.execute("SELECT COUNT(*) FROM users").fetchone()[0], 0)
        self.assertEqual(self.db.execute("SELECT allow_negative_stock FROM store_settings").fetchone()[0], 0)

    def test_file_database_wal_and_connection_settings(self):
        with tempfile.TemporaryDirectory() as folder:
            with closing(sqlite3.connect(str(Path(folder) / "test.db"))) as db:
                execute_sql(db, (ROOT / "src-tauri/db/connection.sql").read_text(encoding="utf-8"))
                self.assertEqual(db.execute("PRAGMA journal_mode").fetchone()[0], "wal")
                self.assertEqual(db.execute("PRAGMA foreign_keys").fetchone()[0], 1)
                self.assertEqual(db.execute("PRAGMA synchronous").fetchone()[0], 2)

    def test_foreign_keys_have_leading_indexes(self):
        missing = []
        tables = self.db.execute("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'product_search%'").fetchall()
        for (table,) in tables:
            pk = sorted((r[5], r[1]) for r in self.db.execute(f'PRAGMA table_info("{table}")') if r[5])
            indexes = [tuple(name for _, name in pk)]
            for index in self.db.execute(f'PRAGMA index_list("{table}")').fetchall():
                if not index[4]:
                    indexes.append(tuple(r[2] for r in self.db.execute(f'PRAGMA index_info("{index[1]}")')))
            groups = {}
            for fk in self.db.execute(f'PRAGMA foreign_key_list("{table}")'):
                groups.setdefault(fk[0], []).append((fk[1], fk[3]))
            for parts in groups.values():
                columns = tuple(name for _, name in sorted(parts))
                if not any(index[:len(columns)] == columns for index in indexes):
                    missing.append((table, columns))
        self.assertEqual(missing, [])

    def test_money_rejects_fractional_storage(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE product_units SET retail_price_minor = 1.5 WHERE id = 1")

    def test_invalid_calendar_dates_rejected(self):
        for value in ("2026-02-30", "2026-13-01", "2026-1-01", "invalid"):
            with self.subTest(value=value), self.assertRaises(sqlite3.IntegrityError):
                self.doc("sale", document_date=value)

    def test_duplicate_barcodes_rejected_across_products(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO product_units(product_id, unit_id, factor_numerator, barcode, retail_price_minor, wholesale_price_minor) VALUES (1, 2, 12, '6220000000001', 1, 1)")

    def test_product_search_tracks_insert_update_delete(self):
        self.assertEqual(self.db.execute("SELECT rowid FROM product_search WHERE product_search MATCH 'ماسوره*'").fetchall(), [(1,)])
        self.db.execute("UPDATE products SET normalized_name = 'محبس' WHERE id = 1")
        self.assertEqual(self.db.execute("SELECT rowid FROM product_search WHERE product_search MATCH 'محبس'").fetchall(), [(1,)])
        self.db.execute("DELETE FROM product_units WHERE id = 1")
        self.db.execute("DELETE FROM products WHERE id = 1")
        self.assertEqual(self.db.execute("SELECT rowid FROM product_search WHERE product_search MATCH 'محبس'").fetchall(), [])

    def test_opening_and_sale_reconcile_stock_and_finance(self):
        self.opening()
        self.sale()
        self.assertEqual(self.db.execute("SELECT quantity_micros, value_minor FROM v_stock_on_hand").fetchall(), [(8000000, 8000)])
        self.assertEqual(self.db.execute("SELECT SUM(debit_minor - credit_minor) FROM v_journal").fetchone()[0], 0)
        self.assertEqual(self.db.execute("SELECT balance_minor FROM v_trial_balance WHERE account_id = 120").fetchone()[0], 8000)

    def test_unbalanced_journal_blocks_posting(self):
        doc = self.doc("manual_journal")
        self.start(doc)
        self.journal(doc, [(101, 100, 0), (301, 0, 99)])
        with self.assertRaisesRegex(sqlite3.IntegrityError, "غير متوازن"):
            self.finish(doc)

    def test_balanced_but_wrong_inventory_journal_rejected(self):
        doc = self.doc("opening_stock", 1000)
        line = self.line(doc, amount=1000)
        self.start(doc)
        self.stock(doc, line, 1000000, 1000, "opening")
        self.journal(doc, [(120, 999, 0), (303, 0, 999)])
        with self.assertRaisesRegex(sqlite3.IntegrityError, "قيمة المخزون"):
            self.finish(doc)

    def test_posted_and_ledger_rows_are_immutable(self):
        doc = self.opening()
        for statement in (
            f"UPDATE documents SET notes = 'changed' WHERE id = {doc}",
            f"DELETE FROM documents WHERE id = {doc}",
            "UPDATE document_lines SET notes = 'changed'",
            "DELETE FROM document_lines",
            "UPDATE stock_movements SET value_minor = 1",
            "DELETE FROM stock_movements",
            "DELETE FROM journal_lines",
            "UPDATE journal_entries SET memo = 'changed'",
        ):
            with self.subTest(sql=statement), self.assertRaises(sqlite3.IntegrityError):
                self.db.execute(statement)

    def test_document_cannot_be_inserted_as_posted(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.doc("manual_journal", status="posted", posted_by=1, posted_at="now")

    def test_stock_cannot_be_inserted_on_draft(self):
        doc = self.doc("opening_stock", 1000)
        line = self.line(doc, amount=1000)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "الترحيل"):
            self.stock(doc, line, 1000000, 1000, "opening")

    def test_negative_stock_default_and_opt_in(self):
        doc = self.doc("sale", 1500)
        line = self.line(doc)
        self.start(doc)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "الرصيد غير كاف"):
            self.stock(doc, line, -1000000, -1000, "sale")
        self.db.execute("UPDATE store_settings SET allow_negative_stock = 1")
        self.stock(doc, line, -1000000, -1000, "sale")
        self.journal(doc, [(101, 1500, 0), (401, 0, 1500), (501, 1000, 0), (120, 0, 1000)])
        self.finish(doc)
        self.assertEqual(self.db.execute("SELECT quantity_micros FROM v_stock_on_hand").fetchone()[0], -1000000)

    def test_failure_rolls_back_entire_transaction(self):
        self.opening()
        before = self.db.execute("SELECT COUNT(*) FROM documents").fetchone()[0]
        self.db.execute("BEGIN IMMEDIATE")
        try:
            doc = self.doc("sale", 3000)
            line = self.line(doc, 2000000, 3000)
            self.start(doc)
            self.stock(doc, line, -2000000, -2000, "sale")
            self.journal(doc, [(101, 3000, 0), (401, 0, 3000), (501, 1999, 0), (120, 0, 2000)])
            self.finish(doc)
            self.fail("Unbalanced posting unexpectedly succeeded")
        except sqlite3.IntegrityError:
            self.db.execute("ROLLBACK")
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM documents").fetchone()[0], before)
        self.assertEqual(self.db.execute("SELECT quantity_micros, value_minor FROM v_stock_on_hand").fetchone(), (10000000, 10000))

    def test_partial_return_and_over_return_limit(self):
        self.opening()
        original, source, _ = self.sale()
        for quantity, should_pass in ((1000000, True), (2000000, False)):
            self.db.execute("BEGIN IMMEDIATE")
            doc = self.doc("sale_return", 1500, source_document_id=original)
            line = self.line(doc, quantity, 1500, source_line_id=source)
            self.start(doc)
            self.stock(doc, line, quantity, 1000, "sale_return")
            self.journal(doc, [(402, 1500, 0), (101, 0, 1500), (120, 1000, 0), (501, 0, 1000)])
            if should_pass:
                self.finish(doc)
                self.db.execute("COMMIT")
            else:
                with self.assertRaisesRegex(sqlite3.IntegrityError, "المرتجع"):
                    self.finish(doc)
                self.db.execute("ROLLBACK")
        self.assertEqual(self.db.execute("SELECT quantity_micros FROM v_stock_on_hand").fetchone()[0], 9000000)

    def test_partial_payment_and_over_allocation(self):
        self.opening()
        _, _, debit = self.sale(credit=True)
        payment = self.doc("receipt_voucher", 1000)
        self.start(payment)
        _, credit = self.journal(payment, [(101, 1000, 0), (110, 0, 1000)])
        self.db.execute("INSERT INTO settlements(document_id, debit_line_id, credit_line_id, amount_minor) VALUES (?, ?, ?, 1000)", (payment, debit, credit))
        self.finish(payment)
        self.assertEqual(self.db.execute("SELECT outstanding_signed_minor FROM v_party_open_items WHERE journal_line_id = ?", (debit,)).fetchone()[0], 2000)
        excess = self.doc("settlement")
        self.start(excess)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "تتجاوز"):
            self.db.execute("INSERT INTO settlements(document_id, debit_line_id, credit_line_id, amount_minor) VALUES (?, ?, ?, 1)", (excess, debit, credit))

    def test_period_lock_blocks_headers_and_details(self):
        doc = self.doc("sale", 1500)
        self.line(doc)
        self.db.execute("UPDATE fiscal_periods SET locked_at = 'now', locked_by = 1 WHERE id = 1")
        for action in (
            lambda: self.start(doc),
            lambda: self.line(doc, line_number=2),
            lambda: self.db.execute("UPDATE document_lines SET notes = 'x'"),
            lambda: self.doc("sale"),
            lambda: self.db.execute("DELETE FROM fiscal_periods WHERE id = 1"),
        ):
            with self.assertRaises(sqlite3.IntegrityError):
                action()

    def test_overlapping_periods_and_hierarchy_cycles_rejected(self):
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("INSERT INTO fiscal_periods(name, starts_on, ends_on) VALUES ('overlap', '2026-12-31', '2027-01-01')")
        self.db.execute("UPDATE categories SET parent_id = 1 WHERE id = 2")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "حلقة"):
            self.db.execute("UPDATE categories SET parent_id = 2 WHERE id = 1")

    def test_stock_backdating_is_rejected(self):
        self.opening()
        doc = self.doc("sale", 1500, document_date="2026-10-26")
        line = self.line(doc)
        self.start(doc)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "أقدم"):
            self.stock(doc, line, -1000000, -1000, "sale")

    def test_transfer_requires_equal_quantities_and_values(self):
        self.opening()
        self.db.execute("INSERT INTO warehouses(id, code, name_ar) VALUES (2, 'SECOND', 'مخزن ثان')")
        doc = self.doc("transfer", destination_warehouse_id=2)
        line = self.line(doc, amount=0)
        self.start(doc)
        self.stock(doc, line, -1000000, -1000, "transfer_out")
        with self.assertRaisesRegex(sqlite3.IntegrityError, "المخزون|التحويل"):
            self.finish(doc)
        self.stock(doc, line, 1000000, 1000, "transfer_in", warehouse=2)
        self.finish(doc)
        self.assertEqual(self.db.execute("SELECT SUM(quantity_micros), SUM(value_minor) FROM v_stock_on_hand").fetchone(), (10000000, 10000))

    def test_currency_scale_is_frozen_after_first_posting(self):
        self.opening()
        with self.assertRaisesRegex(sqlite3.IntegrityError, "عملة"):
            self.db.execute("UPDATE store_settings SET currency_code = 'USD'")

    def test_purchase_invoice_matches_receipt_without_duplicating_quantity(self):
        receipt = self.doc("goods_receipt", 3000)
        received = self.line(receipt, 3000000, 3000)
        self.db.execute("INSERT INTO receipt_lines(document_line_id) VALUES (?)", (received,))
        self.start(receipt)
        self.stock(receipt, received, 3000000, 3000, "receipt")
        self.journal(receipt, [(120, 3000, 0), (210, 0, 3000)])
        self.finish(receipt)
        invoice = self.doc("purchase", 3300)
        billed = self.line(invoice, 3000000, 3300)
        self.db.execute("INSERT INTO purchase_matches(invoice_line_id, receipt_line_id, base_quantity_micros, accrued_value_minor) VALUES (?, ?, 3000000, 3000)", (billed, received))
        self.start(invoice)
        self.stock(invoice, billed, 0, 300, "revaluation")
        self.journal(invoice, [(210, 3000, 0), (120, 300, 0), (201, 0, 3300)])
        self.finish(invoice)
        self.assertEqual(self.db.execute("SELECT quantity_micros, value_minor FROM v_stock_on_hand").fetchone(), (3000000, 3300))
        duplicate = self.doc("purchase", 1000)
        billed_again = self.line(duplicate, amount=1000)
        self.db.execute("INSERT INTO purchase_matches(invoice_line_id, receipt_line_id, base_quantity_micros, accrued_value_minor) VALUES (?, ?, 1000000, 1000)", (billed_again, received))
        self.start(duplicate)
        self.journal(duplicate, [(210, 1000, 0), (201, 0, 1000)])
        with self.assertRaisesRegex(sqlite3.IntegrityError, "فاتورة المورد"):
            self.finish(duplicate)

    def test_partial_purchase_order_receipt_and_over_receipt(self):
        order = self.doc("purchase_order", 5000)
        order_line = self.line(order, 5000000, 5000)
        self.db.execute("UPDATE documents SET status = 'approved' WHERE id = ?", (order,))
        for qty, valid in ((3000000, True), (3000000, False)):
            self.db.execute("BEGIN IMMEDIATE")
            receipt = self.doc("goods_receipt", 3000)
            received = self.line(receipt, qty, 3000)
            self.db.execute("INSERT INTO receipt_lines(document_line_id, purchase_order_line_id) VALUES (?, ?)", (received, order_line))
            self.start(receipt)
            self.stock(receipt, received, qty, 3000, "receipt")
            self.journal(receipt, [(120, 3000, 0), (210, 0, 3000)])
            if valid:
                self.finish(receipt)
                self.db.execute("COMMIT")
            else:
                with self.assertRaisesRegex(sqlite3.IntegrityError, "أمر الشراء"):
                    self.finish(receipt)
                self.db.execute("ROLLBACK")
        self.assertEqual(self.db.execute("SELECT progress, ordered_micros, accepted_micros FROM v_purchase_order_progress").fetchone(), ("partially_received", 5000000, 3000000))

    def test_reversal_requires_exact_original_stock_and_journal(self):
        original = self.opening()
        reversal = self.doc("reversal", reversal_of_id=original, reason="اختبار العكس")
        self.start(reversal)
        with self.assertRaisesRegex(sqlite3.IntegrityError, "العكس|عكس"):
            self.finish(reversal)
        movement = self.db.execute("SELECT id FROM stock_movements WHERE document_id = ?", (original,)).fetchone()[0]
        self.stock(reversal, None, -10000000, -10000, "reversal", reverse=movement)
        entry = self.db.execute("INSERT INTO journal_entries(document_id) VALUES (?)", (reversal,)).lastrowid
        original_lines = self.db.execute("SELECT id, line_number, account_id, debit_minor, credit_minor FROM journal_lines WHERE document_id = ?", (original,)).fetchall()
        for line_id, number, account, debit, credit in original_lines:
            self.db.execute("INSERT INTO journal_lines(entry_id, document_id, line_number, account_id, debit_minor, credit_minor, reverses_line_id) VALUES (?, ?, ?, ?, ?, ?, ?)", (entry, reversal, number, account, credit, debit, line_id))
        self.finish(reversal)
        self.assertEqual(self.db.execute("SELECT quantity_micros, value_minor FROM v_stock_on_hand").fetchone(), (0, 0))
        self.assertEqual(self.db.execute("SELECT balance_minor FROM v_trial_balance WHERE account_id = 120").fetchone()[0], 0)

    def test_aging_boundaries_and_historical_settlements(self):
        as_of = date(2026, 10, 27)
        amount = 100
        line_ids = []
        for days in (-1, 0, 30, 31, 60, 61, 90, 91):
            doc = self.doc("manual_journal", document_date="2026-01-01")
            self.start(doc)
            entry = self.db.execute("INSERT INTO journal_entries(document_id) VALUES (?)", (doc,)).lastrowid
            line_ids.append(self.db.execute("INSERT INTO journal_lines(entry_id, document_id, line_number, account_id, party_id, due_date, debit_minor) VALUES (?, ?, 1, 110, 1, ?, ?)", (entry, doc, (as_of - timedelta(days=days)).isoformat(), amount)).lastrowid)
            self.db.execute("INSERT INTO journal_lines(entry_id, document_id, line_number, account_id, credit_minor) VALUES (?, ?, 2, 301, ?)", (entry, doc, amount))
            self.finish(doc)
        payment = self.doc("receipt_voucher", 100, document_date="2026-10-28")
        self.start(payment)
        _, credit = self.journal(payment, [(101, 100, 0), (110, 0, 100)])
        self.db.execute("INSERT INTO settlements(document_id, debit_line_id, credit_line_id, amount_minor) VALUES (?, ?, ?, 100)", (payment, line_ids[-1], credit))
        self.finish(payment)
        query = (ROOT / "src-tauri/queries/aging.sql").read_text(encoding="utf-8")
        row = self.db.execute(query, {"as_of": as_of.isoformat()}).fetchone()
        self.assertEqual(row[4:], (100, 200, 200, 200, 100, 0, 800))
        next_day = self.db.execute(query, {"as_of": "2026-10-28"}).fetchone()
        self.assertEqual(next_day[-1], 700)

    def test_draft_detail_cannot_be_moved_to_posted_document(self):
        original = self.opening()
        draft = self.doc("sale", 1500)
        line = self.line(draft)
        with self.assertRaises(sqlite3.IntegrityError):
            self.db.execute("UPDATE document_lines SET document_id = ?, line_number = 2 WHERE id = ?", (original, line))

    def test_cancelled_reversal_does_not_block_new_reversal(self):
        original = self.opening()
        cancelled = self.doc("reversal", reversal_of_id=original, reason="خطأ")
        self.db.execute("UPDATE documents SET status = 'cancelled' WHERE id = ?", (cancelled,))
        self.doc("reversal", reversal_of_id=original, reason="تصحيح")


if __name__ == "__main__":
    unittest.main()
