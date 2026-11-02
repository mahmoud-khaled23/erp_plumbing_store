use super::*;

#[test]
fn empty_and_loss_closing_allow_explicit_reopening_and_current_period_reclose() {
    let f = Fixture::new();
    next_period(&f);
    let empty = f.app.mutate(closing(&f, "2026-12-31")).unwrap();
    assert_eq!(account_balance(&f, 302), 0);
    f.app.mutate(reverse(id(&empty), "2027-01-01")).unwrap();
    f.app
        .mutate(request(Mutation::FiscalPeriod {
            id: Some(Int(1)),
            name: "٢٠٢٦".into(),
            starts_on: "2026-01-01".into(),
            ends_on: "2026-12-31".into(),
            locked: false,
            reason: "تصحيح بعد عكس الإقفال".into(),
        }))
        .unwrap();
    f.app
        .mutate(request(Mutation::Journal {
            date: DAY.into(),
            opening: false,
            notes: "مصروف مستحق".into(),
            lines: vec![
                JournalLine {
                    account_id: Int(511),
                    amount: Int(700),
                    party_id: None,
                    due_date: None,
                    memo: String::new(),
                },
                JournalLine {
                    account_id: Int(101),
                    amount: Int(-700),
                    party_id: None,
                    due_date: None,
                    memo: String::new(),
                },
            ],
        }))
        .unwrap();
    let doc = f.app.mutate(closing(&f, "2027-01-02")).unwrap();
    assert_eq!(account_balance(&f, 302), 700);
    assert_eq!(account_balance(&f, 511), 0);
    assert!(
        f.db()
            .execute(
                "UPDATE year_closings SET result_minor=0 WHERE document_id=?1",
                [id(&doc)]
            )
            .is_err()
    );
    assert!(
        f.db()
            .execute("DELETE FROM year_closings WHERE document_id=?1", [id(&doc)])
            .is_err()
    );
    f.reconcile();
}

#[test]
fn bank_match_wrong_account_direction_and_date_leave_no_audit_or_receipt() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 1500));
    f.post(sale);
    let statement_id = id(&f
        .app
        .mutate(request(Mutation::SaveBankStatement {
            statement: statement(vec![-100, 100]),
        }))
        .unwrap());
    let journal: i64 = f
        .db()
        .query_row(
            "SELECT id FROM journal_lines WHERE document_id=?1 AND account_id=101",
            [sale],
            |r| r.get(0),
        )
        .unwrap();
    let other: i64 = f
        .db()
        .query_row(
            "SELECT id FROM journal_lines WHERE document_id=?1 AND account_id=401",
            [sale],
            |r| r.get(0),
        )
        .unwrap();
    let lines = f
        .app
        .query(Query::Reconciliation {
            statement_id: Int(statement_id),
            view: ReconciliationView::Statement,
            limit: 50,
            offset: 0,
        })
        .unwrap();
    let negative = id(&lines["rows"][0]);
    let positive = id(&lines["rows"][1]);
    let before:(i64,i64,i64)=f.db().query_row("SELECT (SELECT COUNT(*) FROM documents),(SELECT COUNT(*) FROM audit_log),(SELECT COUNT(*) FROM command_receipts)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    for (line, j, day) in [
        (negative, journal, DAY),
        (negative, other, DAY),
        (positive, journal, "2026-10-26"),
    ] {
        assert!(
            f.app
                .mutate(request(Mutation::MatchStatement {
                    statement_line_id: Int(line),
                    journal_line_id: Int(j),
                    amount: Int(100),
                    date: day.into(),
                    reason: "اختبار الرفض".into()
                }))
                .is_err()
        );
    }
    let after:(i64,i64,i64)=f.db().query_row("SELECT (SELECT COUNT(*) FROM documents),(SELECT COUNT(*) FROM audit_log),(SELECT COUNT(*) FROM command_receipts)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(before, after);
    assert!(
        f.db()
            .execute(
                "UPDATE bank_statement_lines SET amount_minor=1 WHERE id=?1",
                [positive]
            )
            .is_err()
    );
    assert!(f.db().execute("INSERT INTO bank_statement_lines(statement_id,line_number,value_date,amount_minor,reference,description) VALUES (?1,3,?2,1,'','')",rusqlite::params![statement_id,DAY]).is_err());
    assert!(
        f.db()
            .execute(
                "UPDATE bank_statements SET closing_minor=1 WHERE id=?1",
                [statement_id]
            )
            .is_err()
    );
}

fn next_period(f: &Fixture) {
    f.app
        .mutate(request(Mutation::FiscalPeriod {
            id: None,
            name: "٢٠٢٧".into(),
            starts_on: "2027-01-01".into(),
            ends_on: "2027-12-31".into(),
            locked: false,
            reason: "فترة جديدة".into(),
        }))
        .unwrap();
}
fn closing(f: &Fixture, day: &str) -> Request {
    let p = f
        .app
        .query(Query::ClosingPreview {
            period_id: Int(1),
            date: day.into(),
        })
        .unwrap();
    request(Mutation::CloseYear {
        period_id: Int(1),
        date: day.into(),
        preview_hash: p["preview_hash"].as_str().unwrap().into(),
        reason: "إقفال بعد المراجعة".into(),
    })
}
fn account_balance(f: &Fixture, account: i64) -> i64 {
    f.db()
        .query_row(
            "SELECT COALESCE(SUM(debit_minor-credit_minor),0) FROM v_journal WHERE account_id=?1",
            [account],
            |r| r.get(0),
        )
        .unwrap()
}
fn reverse(id: i64, day: &str) -> Request {
    request(Mutation::Reverse {
        id: Int(id),
        date: day.into(),
        reason: "تصحيح".into(),
    })
}

fn assert_results(f: &Fixture, day: &str, prior: i64, current: i64) {
    let sheet = f
        .app
        .query(Query::Report {
            report: Report::BalanceSheet,
            from: day.into(),
            to: day.into(),
            party_id: None,
            limit: 200,
            offset: 0,
        })
        .unwrap();
    assert_eq!(
        sheet["summary"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["metric"] == "reconciliationDifference")
            .unwrap()["amount_minor"],
        "0"
    );
    for (key, expected) in [("result:prior", prior), ("result:current", current)] {
        assert_eq!(
            sheet["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["drill_key"] == key)
                .unwrap()["balance_minor"],
            expected.to_string()
        );
        let drill = f
            .app
            .query(Query::ReportDrilldown {
                filter: ReportFilter {
                    report: Report::BalanceSheet,
                    from: day.into(),
                    to: day.into(),
                    party_id: None,
                },
                key: key.into(),
                limit: 200,
                offset: 0,
            })
            .unwrap();
        let total: i64 = drill["rows"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                r["credit_minor"].as_str().unwrap().parse::<i64>().unwrap()
                    - r["debit_minor"].as_str().unwrap().parse::<i64>().unwrap()
            })
            .sum();
        assert_eq!(total, expected);
    }
}

#[test]
fn year_close_reversal_reclose_balances_and_keeps_profit_report() {
    let f = Fixture::new();
    next_period(&f);
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 0));
    f.post(sale);
    let close = closing(&f, "2026-12-31");
    let posted = f.app.mutate(close.clone()).unwrap();
    assert_eq!(posted, f.app.mutate(close).unwrap());
    assert_eq!(account_balance(&f, 401), 0);
    assert_eq!(account_balance(&f, 501), 0);
    assert_eq!(account_balance(&f, 302), -500);
    assert_results(&f, "2026-12-31", 0, 0);
    assert!(
        f.app
            .query(Query::ClosingPreview {
                period_id: Int(1),
                date: "2026-12-31".into()
            })
            .is_err()
    );
    assert!(
        f.app
            .mutate(request(Mutation::FiscalPeriod {
                id: Some(Int(1)),
                name: "٢٠٢٦".into(),
                starts_on: "2026-01-01".into(),
                ends_on: "2026-12-31".into(),
                locked: false,
                reason: "تجربة".into()
            }))
            .is_err()
    );
    assert!(f.app.mutate(reverse(id(&posted), "2026-12-31")).is_err());
    assert!(f.app.mutate(reverse(id(&posted), "2028-01-01")).is_err());
    f.app.mutate(reverse(id(&posted), "2027-01-01")).unwrap();
    assert_eq!(account_balance(&f, 401), -1500);
    assert_eq!(account_balance(&f, 501), 1000);
    assert_eq!(account_balance(&f, 302), 0);
    assert_results(&f, "2027-01-01", 500, 0);
    // Next year's operations must not be swallowed by reclosing the previous year.
    let mut later = f.draft(DocumentKind::Sale, SCALE, 1500, 0);
    later.date = "2027-01-02".into();
    let later = f.save(later);
    f.post(later);
    assert_results(&f, "2027-01-02", 500, 500);
    let again = f.app.mutate(closing(&f, "2027-01-03")).unwrap();
    assert_eq!(account_balance(&f, 401), -1500);
    assert_eq!(account_balance(&f, 501), 1000);
    assert_eq!(account_balance(&f, 302), -500);
    assert_ne!(id(&again), id(&posted));
    assert_results(&f, "2027-01-03", 0, 500);
    // Later reversals/recloses must never leak into an earlier as-of date.
    assert_results(&f, "2026-12-31", 0, 0);
    let profit = f
        .app
        .query(Query::Report {
            report: Report::IncomeStatement,
            from: "2026-01-01".into(),
            to: "2027-12-31".into(),
            party_id: None,
            limit: 200,
            offset: 0,
        })
        .unwrap();
    let total: i64 = profit
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            r["profit_signed_minor"]
                .as_str()
                .unwrap()
                .parse::<i64>()
                .unwrap()
        })
        .sum();
    assert_eq!(total, 1000);
    f.reconcile();
}

#[test]
fn closing_rejects_stale_preview_and_locked_next_period_atomically() {
    let f = Fixture::new();
    next_period(&f);
    f.opening();
    let stale = closing(&f, "2026-12-31");
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 0));
    f.post(sale);
    assert!(f.app.mutate(stale).unwrap_err().message.contains("تغيرت"));
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM year_closings", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let posted = f.app.mutate(closing(&f, "2026-12-31")).unwrap();
    f.app
        .mutate(request(Mutation::FiscalPeriod {
            id: Some(Int(2)),
            name: "٢٠٢٧".into(),
            starts_on: "2027-01-01".into(),
            ends_on: "2027-12-31".into(),
            locked: true,
            reason: "قفل".into(),
        }))
        .unwrap();
    assert!(f.app.mutate(reverse(id(&posted), "2027-01-01")).is_err());
    f.reconcile();
}

#[test]
fn customer_adjustments_allocate_reverse_and_never_change_stock_or_vat() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 0));
    f.post(sale);
    let debit:i64=f.db().query_row("SELECT id FROM journal_lines WHERE document_id=?1 AND account_id=110 AND debit_minor>0",[sale],|r|r.get(0)).unwrap();
    let original = f.quantity_value();
    for kind in [
        ReceivableAdjustmentKind::BadDebt,
        ReceivableAdjustmentKind::CustomerCredit,
        ReceivableAdjustmentKind::SettlementDiscount,
    ] {
        let req = request(Mutation::ReceivableAdjustment {
            kind,
            date: DAY.into(),
            debit_line_id: Int(debit),
            amount: Int(500),
            reason: "اتفاق تسوية".into(),
        });
        let doc = f.app.mutate(req.clone()).unwrap();
        assert_eq!(doc, f.app.mutate(req).unwrap());
        assert_eq!(account_balance(&f, 110), 1000);
        assert_eq!(account_balance(&f, 220), 0);
        assert_eq!(f.quantity_value(), original);
        assert!(
            f.app
                .mutate(request(Mutation::ReceivableAdjustment {
                    kind,
                    date: DAY.into(),
                    debit_line_id: Int(debit),
                    amount: Int(1001),
                    reason: "زيادة".into()
                }))
                .is_err()
        );
        assert!(f.app.mutate(reverse(sale, DAY)).is_err());
        f.app.mutate(reverse(id(&doc), DAY)).unwrap();
        assert_eq!(account_balance(&f, 110), 1500);
        assert_eq!(f.db().query_row("SELECT outstanding_signed_minor FROM v_party_open_items WHERE journal_line_id=?1",[debit],|r|r.get::<_,i64>(0)).unwrap(),1500);
    }
    f.reconcile();
}

fn statement(lines: Vec<i64>) -> BankStatement {
    BankStatement {
        cash_account_id: Int(1),
        starts_on: DAY.into(),
        ends_on: DAY.into(),
        reference: "كشف ١".into(),
        opening: Int(0),
        closing: Int(lines.iter().sum()),
        lines: lines
            .into_iter()
            .map(|n| BankStatementLine {
                date: DAY.into(),
                amount: Int(n),
                reference: "إيداع".into(),
                description: String::new(),
            })
            .collect(),
    }
}
#[test]
fn bank_matching_partial_immutable_undo_and_reversal_guard() {
    let f = Fixture::new();
    f.opening();
    let sale = f.save(f.draft(DocumentKind::Sale, SCALE, 1500, 1500));
    f.post(sale);
    let bank = f
        .app
        .mutate(request(Mutation::SaveBankStatement {
            statement: statement(vec![1000, 500]),
        }))
        .unwrap();
    let journal: i64 = f
        .db()
        .query_row(
            "SELECT id FROM journal_lines WHERE document_id=?1 AND account_id=101",
            [sale],
            |r| r.get(0),
        )
        .unwrap();
    let rows = f
        .app
        .query(Query::Reconciliation {
            statement_id: Int(id(&bank)),
            view: ReconciliationView::Statement,
            limit: 50,
            offset: 0,
        })
        .unwrap();
    let line = rows["rows"][0]["id"]
        .as_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();
    let req = request(Mutation::MatchStatement {
        statement_line_id: Int(line),
        journal_line_id: Int(journal),
        amount: Int(600),
        date: DAY.into(),
        reason: "مطابقة".into(),
    });
    let doc = f.app.mutate(req.clone()).unwrap();
    assert_eq!(doc, f.app.mutate(req).unwrap());
    assert!(
        f.app
            .mutate(reverse(sale, DAY))
            .unwrap_err()
            .message
            .contains("المطابقة")
    );
    assert!(
        f.app
            .mutate(request(Mutation::MatchStatement {
                statement_line_id: Int(line),
                journal_line_id: Int(journal),
                amount: Int(401),
                date: DAY.into(),
                reason: "زيادة".into()
            }))
            .is_err()
    );
    let matches = f
        .app
        .query(Query::Reconciliation {
            statement_id: Int(id(&bank)),
            view: ReconciliationView::Matches,
            limit: 50,
            offset: 0,
        })
        .unwrap();
    let match_id = id(&matches["rows"][0]);
    assert!(
        f.db()
            .execute("DELETE FROM reconciliation_matches WHERE id=?1", [match_id])
            .is_err()
    );
    assert!(
        f.app
            .mutate(request(Mutation::VoidBankStatement {
                statement_id: Int(id(&bank)),
                date: DAY.into(),
                reason: "إلغاء".into()
            }))
            .is_err()
    );
    f.app
        .mutate(request(Mutation::UnmatchStatement {
            match_id: Int(match_id),
            date: DAY.into(),
            reason: "تصحيح".into(),
        }))
        .unwrap();
    assert!(
        f.app
            .mutate(request(Mutation::UnmatchStatement {
                match_id: Int(match_id),
                date: DAY.into(),
                reason: "مكرر".into()
            }))
            .is_err()
    );
    f.app.mutate(reverse(sale, DAY)).unwrap();
    assert!(
        f.app
            .mutate(request(Mutation::MatchStatement {
                statement_line_id: Int(line),
                journal_line_id: Int(journal),
                amount: Int(100),
                date: DAY.into(),
                reason: "معكوس".into()
            }))
            .is_err()
    );
    f.app
        .mutate(request(Mutation::VoidBankStatement {
            statement_id: Int(id(&bank)),
            date: DAY.into(),
            reason: "كشف خاطئ".into(),
        }))
        .unwrap();
    f.reconcile();
}

#[test]
fn bank_csv_exact_numbers_duplicate_and_invalid_statement_rollback() {
    let f = Fixture::new();
    let data=f.app.query(Query::ParseStatementCsv{csv:format!("\u{feff}date,amount,reference,description\n{DAY},١٢٫٣٤,A,إيداع\n{DAY},-0.01,B,سحب\n")}).unwrap();
    assert_eq!(data[0]["amount"], "1234");
    assert_eq!(data[1]["amount"], "-1");
    assert!(
        f.app
            .query(Query::ParseStatementCsv {
                csv: format!("date,amount,reference,description\n{DAY},1.001,A,B\n")
            })
            .is_err()
    );
    let mut bad = statement(vec![100]);
    bad.closing = Int(99);
    assert!(
        f.app
            .mutate(request(Mutation::SaveBankStatement { statement: bad }))
            .is_err()
    );
    let good = statement(vec![100]);
    let r = request(Mutation::SaveBankStatement {
        statement: good.clone(),
    });
    let doc = f.app.mutate(r.clone()).unwrap();
    assert_eq!(doc, f.app.mutate(r).unwrap());
    let mut duplicate = good;
    duplicate.reference = "اسم آخر".into();
    assert!(
        f.app
            .mutate(request(Mutation::SaveBankStatement {
                statement: duplicate
            }))
            .is_err()
    );
    assert_eq!(
        f.db()
            .query_row("SELECT COUNT(*) FROM bank_statements", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    f.app
        .create_user("cashier", "أمين الصندوق", "CashierPassword12", &[Int(2)])
        .unwrap();
    f.app.login("cashier", "CashierPassword12").unwrap();
    assert_eq!(
        f.app
            .query(Query::BankStatements {
                after_id: Int(0),
                limit: 50
            })
            .unwrap_err()
            .code,
        "forbidden"
    );
    assert_eq!(
        f.app
            .mutate(request(Mutation::SaveBankStatement {
                statement: statement(vec![200])
            }))
            .unwrap_err()
            .code,
        "forbidden"
    );
}
