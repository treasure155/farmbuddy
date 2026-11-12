-- FarmBuddy 1.0.2: its own identity.
-- New default colours (emerald actions, deep forest navigation) replace the
-- earlier defaults, but never a colour the owner chose.
UPDATE settings SET value = '#17845A' WHERE key = 'theme_primary' AND value IN ('#E07A1F', '#E8731A');
UPDATE settings SET value = '#0F2E24' WHERE key = 'theme_secondary' AND value IN ('#1E4D2B', '#1D4E89');
-- There are no shop, eatery or laundry business types in FarmBuddy.
UPDATE settings SET value = 'farm' WHERE key = 'business_type';
-- Farm wording where the old shop wording was never changed.
UPDATE settings SET value = 'Farm sales receipt' WHERE key = 'receipt_title' AND value IN ('Sales receipt', 'Receipt');
UPDATE settings SET value = 'Thank you for buying from our farm.' WHERE key = 'receipt_footer' AND value = 'Thank you for your patronage.';
UPDATE settings SET value = 'Buyer' WHERE key = 'label_customer' AND value = 'Customer';
