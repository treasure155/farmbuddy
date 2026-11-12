/** The user guide shown on the How to page. Plain HTML, written for farmers, farm staff and owners. */

export interface GuideSection { id: string; title: string; who?: string; html: string }

const table = (head: string[], rows: string[][]) =>
  `<div class="table-scroll"><table class="table guide-table"><thead><tr>${head.map((h) => `<th>${h}</th>`).join('')}</tr></thead><tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${c}</td>`).join('')}</tr>`).join('')}</tbody></table></div>`;

export const GUIDE: GuideSection[] = [
  {
    id: 'about', title: 'What FarmBuddy does',
    html: `<p class="guide-lead">FarmBuddy runs your farm business on this computer, without internet: livestock, poultry and fish, crops, processing, feed, health, storage, sales of your produce, expenses and reports, with suggestions worked out from your own records. It works in English, and Ibibio is being added screen by screen.</p>
    ${table(['Area', 'What you can do'], [
      ['Farm setup', 'Guided questions create your farm, switch on only the pages you need and create your first batches and plots.'],
      ['Livestock and fish', 'Poultry, goats, sheep, pigs, cattle, rabbits, snails and fish in batches: deaths, culls, feed, water, eggs, weights, costs, sales and profit.'],
      ['Feed', 'Feed guides with their source, expected against actual feed, wastage, FCR, comparison across batches and a feed-mix calculator.'],
      ['Health and vaccination', 'Vaccination plans with Due, Overdue and Needs vet review, and health records with what a vet prescribed. No diagnosis.'],
      ['Crop farming', 'Fields and plots, planting, field work with labour and inputs from stock, harvests, yield, losses, a calendar and crop health.'],
      ['Processing', 'Palm oil, garri, flour and feed as costed batches; products go into stock at their cost per litre or kg.'],
      ['Storage', 'Stores, warehouses and cold rooms: put away, issue, transfer, spoilage, moisture checks and stock by location.'],
      ['Weather and markets', 'Rainfall records, NiMet seasonal outlooks you enter, and market price history.'],
      ['Farm advisory', 'Suggestions from your records, Ask FarmBuddy check-ups, reminders, and an offline library with sources and review status.'],
      ['Selling produce', 'Sell birds, eggs, fish, crops and processed products: quantities like 2.5 kg, discounts, a named buyer, cash, transfer, POS or credit.'],
      ['Receipts', 'Print on 58 mm or 80 mm paper or on A4, send on WhatsApp or email, record later payments and void sales with a reason.'],
      ['Farm stock', 'Feed, drugs, seed, fertiliser and produce, with reorder warnings, expiry dates, stock in and out, counts and full history.'],
      ['Purchases and suppliers', 'Record deliveries from hatcheries, feed mills and input dealers, and what you still owe each one.'],
      ['Money', 'Expenses, farm profit and loss, cash flow, profit by batch or crop, and a finance summary for loans and grants.'],
      ['People', 'Owner, Manager, Cashier, Adviser and Farm worker accounts. Workers and advisers never see prices or profit.'],
      ['Backups and recovery', 'Automatic daily backups; forgot password and new owner, protected by a recovery key. No internet needed.'],
    ])}
    <h3>Who it is for</h3>
    <p>Poultry, livestock, fish and crop farmers, oil palm and cassava producers, processors, produce traders and farm input shops. Setup asks what you manage and loads that farm type's starter items (feeds, vaccines, seed, produce) by name only. You enter your own prices and stock.</p>`,
  },
  {
    id: 'navigate', title: 'Finding your way around',
    html: `
      <p>On a computer, the menu on the left lists every page, grouped as <b>Overview</b>, <b>Farm</b>, <b>Advice</b>, <b>Sell and stock</b>, <b>Money</b> and <b>Business</b>. The farm you are working on is shown at the top of the menu; click it to switch farms.</p>
      <p><b>Today</b> shows your farm at a glance: live animals, vaccinations due, crops growing, processing, suggestions, reminders and, for managers, money this month.</p>
      <p>On a phone, the bar at the bottom takes you to Today, Sell, Farm, Advice and Menu (every page). The arrow at the top left, or the phone's back button, returns to the previous page.</p>
      <p><kbd>F2</kbd> opens Sell produce. <b>About &amp; contact</b> (tap your initials) shows the app version, your licence and how to reach TechAlpha Hub.</p>`,
  },
  {
    id: 'setup', title: 'First-time setup',
    html: `<p class="guide-lead">The first time FarmBuddy opens, it asks about your farm, then creates the owner account. This is done once per computer.</p>
    <ol>
      <li><b>Language:</b> English or Ibibio. Change it later in My Farm.</li>
      <li><b>Your farm:</b> name, state, LGA, community, phone, address and the type of agricultural business.</li>
      <li><b>What you manage:</b> poultry, goats, fish, crops, oil palm, cassava, processing and so on. Only the questions for your choices follow.</li>
      <li><b>Size:</b> how many birds or animals, ponds, how much land, what you process and who you sell to.</li>
      <li><b>How you want to work:</b> purchases, sales and receipts, stock, costs, workers, reminders, suppliers and buyers, reports.</li>
      <li><b>Owner account:</b> your full name, a username and a password of at least 8 characters.</li>
      <li><b>Account recovery:</b> three security questions, then a 16-character recovery key such as <code>K7QM-4XPA-9TRB-2HWN</code>. Print it or write it down.</li>
    </ol>
    <p>FarmBuddy then creates your farm, your first batches and plots, and the starter items for your farm type, with no prices or stock.</p>
    <div class="guide-note"><b>Keep the recovery key private.</b> Anyone holding it can reset passwords and create an owner account.</div>
    <h3>Before you start recording</h3>
    <ul class="guide-checks">
      <li><b>Settings → Farm profile:</b> address, phone, email and logo.</li>
      <li><b>Farm stock:</b> enter prices and opening stock for your feeds, drugs and produce.</li>
      <li><b>Settings → Payment methods:</b> add OPay, Moniepoint and so on.</li>
      <li><b>Staff:</b> add an account for each manager, farm worker, adviser and cashier.</li>
    </ul>`,
  },
  {
    id: 'signin', title: 'Signing in, forgot password and new owner',
    html: `<p class="guide-lead">Everyone signs in with their own username and password. If a password is forgotten, the sign-in screen can reset it without internet.</p>
    <h3>Signing in and out</h3>
    <ul>
      <li>Type your username and password and click <b>Sign in</b>. <b>Show</b> reveals the password as you type.</li>
      <li>Sign out with the arrow beside your name at the bottom of the menu.</li>
      <li>After 5 wrong passwords, that username is blocked for 1 minute. After 12 hours without use, the app signs out by itself.</li>
    </ul>
    <h3>Forgot password</h3>
    <ol>
      <li>On the sign-in screen, click <b>Forgot password?</b></li>
      <li>Choose how to prove it is you: the <b>Recovery key</b>, or the answers to your three <b>Security questions</b>.</li>
      <li>Type the username of the account to reset. It can be any account: owner, manager or cashier.</li>
      <li>Choose a new password, type it twice, and click <b>Reset password and sign in</b>.</li>
    </ol>
    <p>If you used the recovery key, the app now shows a <b>new key</b>, because the old one has been used. Save the new key before you continue.</p>
    <h3>Create a new owner account</h3>
    <p>Use this when nobody can sign in as owner, for example when the owner has left or the account is lost.</p>
    <ol>
      <li>Click <b>Create a new owner account</b> on the sign-in screen.</li>
      <li>Prove ownership with the recovery key or the security answers.</li>
      <li>Enter the new owner's full name, username and password, then click <b>Create owner and sign in</b>.</li>
    </ol>
    <div class="guide-note">A cashier cannot use these screens to take over the business, because both need the recovery key or all three security answers. After 5 wrong tries, recovery is locked for 10 minutes, and every attempt is listed under <b>Settings → Security &amp; recovery</b>.</div>
    <h3>Changing the key or questions (Owner)</h3>
    <p>Open <b>Settings → Security &amp; recovery</b>. You can create a new recovery key (the old one stops working at once) or change the security questions. Both need your current password. The same page lists recent recovery activity, including failed attempts.</p>`,
  },
  {
    id: 'staff', title: 'Staff accounts and roles', who: 'Owner',
    html: `<p class="guide-lead">Give every person their own account, so each record shows who made it.</p>
    ${table(['What they can do', 'Farm worker', 'Adviser', 'Cashier', 'Manager', 'Owner'], [
      ['Daily farm records: deaths, feed, eggs, field work, health', '✓', '✓', '✓', '✓', '✓'],
      ['Confirm health records and review schedules', '', '✓', '', '✓', '✓'],
      ['Sell produce, receipts, buyers', '', '', '✓', '✓', '✓'],
      ['See prices, costs and profit', '', '', 'Sales only', '✓', '✓'],
      ['Farm stock, purchases, expenses and reports', '', '', '', '✓', '✓'],
      ['Staff, settings, recovery, backups', '', '', '', '', '✓'],
    ])}
    <h3>Adding staff</h3>
    <ol><li>Open <b>Staff</b> and click <b>Add staff</b>.</li><li>Enter their full name and a username, and choose the role. You can limit them to chosen farms.</li><li>Set a password of at least 8 characters and tell them privately.</li></ol>
    <h3>Changes</h3>
    <ul>
      <li><b>Reset a password:</b> Staff → Edit → type a new password → Save changes.</li>
      <li><b>Change a role:</b> Staff → Edit → choose the role. They are signed out and must sign in again.</li>
      <li><b>Someone leaves:</b> Staff → Edit → untick <b>Account active</b>. Their records stay.</li>
    </ul>
    <p>Each person changes their own password under <b>Settings → My account</b>.</p>`,
  },
  {
    id: 'sale', title: 'Making a sale',
    html: `<p class="guide-lead">Open <b>New sale</b> from the menu or press <kbd>F2</kbd> anywhere. Items are on the left and the current sale is on the right.</p>
    <figure class="guide-shot"><img src="/guide/new-sale.png" alt="The Sell produce screen with items on the left and the sale on the right"><figcaption>Sell produce. The sale builds up on the right.</figcaption></figure>
    <h3>Adding items</h3>
    <ul>
      <li><b>Tap an item</b> to add one. The small number on it shows how many are in the sale.</li>
      <li><b>Scan a barcode.</b> The item is added as soon as the scan finishes (see Barcode scanning).</li>
      <li><b>Search</b> by name or code and press <kbd>Enter</kbd>. If only one item matches, it is added.</li>
      <li>Use the <b>category buttons</b> to show one group, such as Beverages.</li>
    </ul>
    <p>Items running low show their stock in orange. Out-of-stock items are greyed out.</p>
    <h3>Changing the sale</h3>
    <ul>
      <li><b>Quantity:</b> the − and + buttons, or click the number and type, for example 2.5.</li>
      <li><b>Remove:</b> the × beside the line.</li>
      <li><b>Price:</b> click the price on the line (managers and owners, or cashiers if the owner allows it).</li>
      <li><b>Discount:</b> <b>Add discount</b>, then enter an amount or a percentage.</li>
      <li><b>Customer:</b> <b>Add customer</b> to choose or add one. This is needed for credit sales.</li>
      <li><b>Clear sale</b> at the top right starts over.</li>
    </ul>
    <p>The sale is kept if you move to another page. When ready, click <b>Take payment</b> or press <kbd>F9</kbd>.</p>`,
  },
  {
    id: 'barcode', title: 'Barcode scanning',
    html: `<p class="guide-lead">Any USB or Bluetooth barcode scanner that types like a keyboard works. Most do this out of the box, and send Enter after each scan.</p>
    <h3>Selling with a scanner</h3>
    <ol><li>Open <b>New sale</b>. The search box is ready for scanning.</li><li>Scan the product. It is added to the sale straight away.</li><li>Scan again to add another one.</li></ol>
    <h3>Giving products their barcodes</h3>
    <ul>
      <li><b>From Inventory:</b> open the item, click in the <b>Barcode</b> box and scan the product. Save.</li>
      <li><b>From Sell produce (Manager or Owner):</b> scan a product that has no barcode saved yet. The app asks <b>Add it as a new item now?</b> Click it, fill in the name and price (the barcode is already filled in), and save. It is added to the sale at once.</li>
    </ul>
    <div class="guide-note">When adding a new stocked item from Sell produce, enter its <b>opening stock</b>. With zero stock the app will not sell it unless the owner allows selling at zero stock.</div>
    <p>If a cashier scans an unknown code, the app says no item has that code. A manager can then add it.</p>`,
  },
  {
    id: 'payment', title: 'Taking payment',
    html: `<p class="guide-lead">The payment window shows the amount due. The first payment method is already filled in with the full amount.</p>
    <h3>A normal sale</h3>
    <ol><li>Click the method the customer is paying with, for example <b>Bank transfer</b>. The full amount moves to it.</li><li>Optionally type a reference, such as the last digits of the transfer.</li><li>Click <b>Complete sale</b>.</li></ol>
    <h3>Cash and change</h3>
    <ol><li>Choose <b>Cash</b>.</li><li>Click an amount under <b>Cash received</b> (Exact, ₦7,000, ₦10,000…) or type what the customer gave you.</li><li>The app shows <b>Change to give</b>. It also prints on the receipt.</li></ol>
    <h3>Split payment</h3>
    <p>For example, ₦5,000 cash and the rest by transfer: click <b>Cash</b> and type 5000, then click <b>Bank transfer</b>, which fills in what is left. Click <b>Complete sale</b>.</p>
    <h3>Credit and part payment</h3>
    <ul>
      <li>Take less than the total, or remove all payment lines. The app shows <b>Balance owed</b>.</li>
      <li>A sale that is not fully paid needs a named customer.</li>
      <li>Record later payments from the receipt (see Receipts).</li>
    </ul>
    <h3>After the sale</h3>
    <p>Click <b>Print receipt</b> and choose your receipt printer, or type the customer's email and click <b>Send</b>. <b>Start next sale</b> closes the window.</p>`,
  },
  {
    id: 'receipts', title: 'Receipts',
    html: `<p class="guide-lead">The <b>Receipts</b> page lists every sale, newest first. Cashiers see only their own.</p>
    <ul>
      <li><b>Search</b> by receipt number, customer name, phone or email, and filter by payment status and dates.</li>
      <li>Click a receipt to open it on the right, where you can <b>Print</b> or <b>Email</b> it again.</li>
    </ul>
    ${table(['Status', 'Meaning'], [['Paid', 'Fully paid.'], ['Part paid', 'Some money received. A balance is owed.'], ['Owing', 'Nothing paid yet.'], ['Void', 'Cancelled. Stock was returned and the sale is left out of reports.']])}
    <h3>Collecting a balance</h3>
    <ol><li>Open the receipt and click <b>Record payment</b>.</li><li>Enter the amount, choose the method and click <b>Save payment</b>.</li></ol>
    <h3>Voiding (Manager or Owner)</h3>
    <p>Open the receipt, click <b>Void receipt</b>, type a reason and confirm. Stock goes back and the sale leaves the reports. Receipts are never deleted.</p>`,
  },
  {
    id: 'inventory', title: 'Inventory', who: 'Manager or Owner',
    html: `<p class="guide-lead">The <b>Inventory</b> page holds your items, categories and stock history.</p>
    ${table(['Kind', 'Use it for', 'Sold?', 'Stock counted?'], [
      ['For sale', 'Birds, eggs, fish, crops, palm oil, garri', 'Yes', 'Yes'],
      ['Service', 'Washing a shirt, delivery, dressing a bird', 'Yes', 'No'],
      ['Farm input', 'Feed, vaccines, drugs, seed, fertiliser', 'No', 'Yes'],
    ])}
    <h3>Adding or editing an item</h3>
    <ol>
      <li>Click <b>Add item</b> (or the pencil beside an item to edit it).</li>
      <li>Choose the kind, then enter the name, category and unit (piece, pack, kg, plate, bird, bag…).</li>
      <li>Optionally scan its <b>barcode</b> into the Barcode box.</li>
      <li>Enter the selling price and the cost price. The app shows your margin.</li>
      <li>For a new stocked item, enter the opening stock and a reorder level.</li>
      <li>Click <b>Save</b>, or <b>Save and add another</b>.</li>
    </ol>
    <p><b>Delete</b> removes an item that was never sold. Items that have been sold are archived instead, so old receipts stay correct.</p>
    <h3>Changing stock</h3>
    ${table(['Action', 'When', 'You enter'], [
      ['Receive', 'A delivery arrives or a batch is ready', 'Quantity, and optionally the cost per unit (updates your average cost)'],
      ['Remove', 'Died or culled, used on the farm, spoilt, damaged, expired', 'Quantity and a reason (required)'],
      ['Count', 'After counting the store', 'The exact amount counted'],
    ])}
    <p>Click <b>Stock</b> beside the item. The <b>Stock history</b> tab lists every change with who made it.</p>`,
  },
  {
    id: 'licence', title: 'Free trial and licence', who: 'Owner',
    html: `<p class="guide-lead">Every new computer gets a <b>14-day free trial</b> with all features. To keep using FarmBuddy after that, each computer needs its own licence key.</p>
    <ol>
      <li>Open <b>Settings → Licence</b> (or click <b>Activate</b> in the blue strip at the top).</li>
      <li>Copy the <b>Install ID</b>, for example <code>G147-RQYW-Y4VJ-4EQK</code>, and send it to TechAlpha Hub: click <b>Request key on WhatsApp</b> (+234 706 615 5981) or <b>Email us</b> (info@techalphahub.online). The Install ID is filled in for you. A licence costs ₦30,000 per year or ₦3,000 per month.</li>
      <li>TechAlpha Hub sends back a licence key starting with <code>ESTK-</code>. Paste it into the box and click <b>Activate</b>.</li>
    </ol>
    <p>A key only works on the computer whose Install ID it was made for. A second shop computer needs its own key. If the trial or a dated licence ends, the app shows the activation screen; your sales, stock and customers are kept safe and come back as soon as you activate.</p>
    <p>During the trial, and in the last week of a dated licence, a reminder shows once a day with the days left and the WhatsApp and email buttons. Tap <b>Remind me tomorrow</b> to close it for the day.</p>
    <p class="guide-note">Setting the computer clock backwards does not extend the trial, and reinstalling the app does not restart it.</p>`,
  },
  {
    id: 'packs', title: 'Bags, crates and wholesale prices', who: 'Manager or Owner',
    html: `<p class="guide-lead">One item can be sold singly or by the bag or crate, and big buyers can get a wholesale price automatically.</p>
    <ol>
      <li>In <b>Inventory</b>, edit the item and open <b>Bags, crates and wholesale prices</b>.</li>
      <li>Fill in the pack name (bag, crate, basin), how many single units are in one pack, and the pack price.</li>
      <li>For wholesale, enter the wholesale price per unit and the quantity from which it applies, for example ₦280 from 10 pieces.</li>
    </ol>
    <p>On Sell produce, the item's tile shows a small <b>+ bag</b> (or crate) button. Tap the tile for one unit, or the button for one bag. On the sale, tap the unit to switch a line between single units and bags. Stock is always counted in single units, so selling one bag of 50 kg takes 50 kg from stock.</p>
    <p>When a line reaches the wholesale quantity, the price changes by itself and the line is marked <b>Wholesale</b>.</p>`,
  },
  {
    id: 'held', title: 'Holding a sale', 
    html: `<p class="guide-lead">Park a sale to serve someone else, then bring it back later. Useful when a buyer goes to get more money or to bring a vehicle.</p>
    <ol><li>On the ticket, click <b>Hold this sale</b> and give it a name, such as "Table 4" or "Mrs Bello".</li>
    <li>To continue it, click <b>Held sales</b> at the top of New sale and choose it.</li></ol>
    <p>Stock is only taken when the held sale is finally completed. Held sales can be seen by every cashier on this computer.</p>`,
  },
  {
    id: 'shifts', title: 'Shifts and cash-up',
    html: `<p class="guide-lead">A shift tells you whether the cash in the drawer is right at the end of the day.</p>
    <ol>
      <li><b>Open a shift:</b> New sale → <b>Open shift</b>, or the <b>Shifts &amp; cash-up</b> page. Count the change (float) already in the drawer and enter it.</li>
      <li>Sell as usual. Cash refunds and cash expenses paid during the shift are counted automatically.</li>
      <li><b>Close the shift:</b> Shifts &amp; cash-up → <b>Close shift &amp; count cash</b>. Count all the cash in the drawer and enter it. The app shows <b>Balanced</b>, <b>Short</b> or <b>Over</b>.</li>
      <li>A shift report (Z report) opens. Print it and keep it with the cash.</li>
    </ol>
    ${table(['Expected cash is', ''], [['Opening float', '+'], ['Cash taken in sales and old balances', '+'], ['Cash refunds given', '−'], ['Cash expenses paid', '−']])}
    <p>Managers see every cashier's shifts on the same page, with shortages in red. The owner can make shifts compulsory in <b>Settings → Selling rules &amp; wording → Cashiers must open a shift before selling</b>.</p>`,
  },
  {
    id: 'returns', title: 'Returns and refunds', who: 'Manager or Owner',
    html: `<p class="guide-lead">Take back some or all of the items on a receipt, without cancelling the whole sale.</p>
    <ol>
      <li>Open the receipt in <b>Receipts</b> and click <b>Return items</b>.</li>
      <li>Enter how many of each item came back, choose how the refund is paid, and give a reason.</li>
      <li>Leave <b>Put the goods back in stock</b> ticked if they can be sold again. Untick it for spoilt or damaged goods.</li>
    </ol>
    <p>If the customer still owed money on the receipt, the returned value comes off what they owe first. Only money they had already paid beyond the new total is refunded. The app tells you the exact amount to pay back.</p>
    <p><b>Void</b> is still there for a sale entered by mistake, but once anything on a receipt has been returned, use Return for the rest.</p>`,
  },
  {
    id: 'whatsapp', title: 'WhatsApp receipts and statements',
    html: `<p class="guide-lead">Send a receipt or a statement of account straight to a customer's WhatsApp.</p>
    <ul>
      <li>After a sale, click <b>Send on WhatsApp</b>. On an old receipt, click <b>WhatsApp</b>.</li>
      <li>In <b>Customers</b>, click <b>Statement</b>, choose the dates, then <b>WhatsApp</b> or <b>Print</b>. The statement lists sales, payments, returns and the balance due.</li>
    </ul>
    <p>WhatsApp (or WhatsApp Web) opens with the message ready; you press Send. If the customer has a phone number saved, their chat opens directly. Numbers like 0803 123 4567 are changed to +234 803 123 4567 automatically; change the country code in Settings → Selling rules &amp; wording.</p>`,
  },
  {
    id: 'purchases', title: 'Purchases and suppliers', who: 'Manager or Owner',
    html: `<p class="guide-lead">Record goods received from suppliers. Stock goes up, cost prices stay correct and you can see what you still owe.</p>
    <ol>
      <li>Open <b>Purchases</b> and click <b>Receive goods</b>.</li>
      <li>Type the supplier's name (new suppliers are saved for you) and the invoice or waybill number.</li>
      <li>Add each item with its quantity and cost. Items with packs can be received by the bag or crate. Items that track expiry ask for a batch number and expiry date.</li>
      <li>Enter how much you paid now. Anything less than the total is recorded as owed to the supplier.</li>
    </ol>
    <p>To pay a supplier later, open the purchase and use <b>Record payment</b>. The <b>Suppliers</b> tab shows what you have bought from each one and what you owe.</p>
    <p class="guide-note">The cost price becomes the average of old and new stock, so profit in reports stays right when buying prices change.</p>`,
  },
  {
    id: 'expenses', title: 'Expenses and net profit', who: 'Manager or Owner',
    html: `<p class="guide-lead">Record what the business spends to see your real profit.</p>
    <ol><li>Open <b>Expenses</b> and click <b>Record expense</b>.</li><li>Enter the date, amount and category (diesel, salaries, rent…), and who it was paid to.</li></ol>
    <p>Reports show <b>Gross profit</b> (sales minus the cost of goods sold) and <b>Net profit</b> (gross profit minus expenses). Cash expenses paid during a shift are taken off the cash expected in the drawer.</p>`,
  },
  {
    id: 'expiry', title: 'Expiry dates and batches', who: 'Manager or Owner',
    html: `<p class="guide-lead">For milk, drugs, vaccines, frozen food and anything else that expires.</p>
    <ol>
      <li>Edit the item and tick <b>Track expiry dates</b>.</li>
      <li>When stock arrives (Stock → Receive, or Purchases), enter the batch number and expiry date.</li>
      <li>Sales take from the batch that expires first.</li>
    </ol>
    <p>The Overview warns you about batches expiring within 30 days (change the number in Settings → Selling rules &amp; wording). <b>Inventory → Expiry dates</b> lists every batch; expired batches have a <b>Remove from stock</b> button.</p>`,
  },
  {
    id: 'farm-setup', title: 'Setting up your farm', who: 'Owner',
    html: `<p class="guide-lead">The first time the owner signs in, FarmBuddy asks a few questions and prepares the pages you need.</p>
    <ol>
      <li>Choose <b>English</b> or <b>Ibibio</b>.</li>
      <li>Enter your farm name, state, LGA and the type of business.</li>
      <li>Tick everything you manage: poultry, goats, fish, crops, oil palm, processing…</li>
      <li>Answer the questions that follow. They depend on your answers: a poultry farmer is never asked about crops.</li>
      <li>Choose what you want help with: purchases, sales, stock, costs, reminders and reports.</li>
    </ol>
    <p>FarmBuddy creates your farm, your first livestock batches and your first plot. Change anything later in <b>Farm → My Farm</b>, where you can also run the setup again, switch pages on or off, and load or remove the <b>demo farm</b>.</p>`,
  },
  {
    id: 'livestock', title: 'Livestock, poultry and fish', who: 'Farm staff, Manager or Owner',
    html: `<p class="guide-lead">Follow each batch from placement to sale.</p>
    <ol>
      <li>On <b>Livestock</b> click <b>New batch</b>: the type (broilers, layers, goats, catfish…), code, date placed, number placed and what they cost.</li>
      <li>Every day, open the batch and add a <b>Daily record</b>: deaths, culls, feed given, water, eggs for layers and, when you weigh a sample, the average weight.</li>
      <li>Add <b>costs</b> (feed, vaccines, labour). Choose a stock item to take it out of inventory at the same time.</li>
      <li>Record <b>sales</b> with the number sold, total live weight and amount received.</li>
    </ol>
    ${table(['Figure', 'How FarmBuddy works it out'], [['Mortality', 'Deaths ÷ all animals that came in × 100 (placed, received, born and moved in). Never the number alive today.'], ['Survival', '100 − mortality: animals that did not die, including those sold or moved'], ['FCR', 'Feed used ÷ live weight produced (weight sold + animals alive × latest average weight). Needs a recent weighing.'], ['Cost per animal', 'All costs ÷ animals sold and still alive'], ['Profit', 'Money from sales minus purchase cost and all other costs']])}
    <p>Deaths, culls and sales are separate entries, so an animal is never taken off twice. Ask your feed supplier or extension officer what figures to expect for your breed. Close the batch when the last animals are sold.</p>`,
  },
  {
    id: 'health', title: 'Health and vaccination', who: 'Farm staff, Manager or Owner',
    html: `<p class="guide-lead">Keep a clear health history you can show your vet. FarmBuddy does not diagnose disease or prescribe medicine.</p>
    <ol>
      <li><b>Plan vaccination</b>: batch, date, vaccine, and where the schedule came from (your vet, the vaccine label, the hatchery, or a general reference).</li>
      <li>A plan from a general reference shows <b>Needs vet review</b> until you record who confirmed it.</li>
      <li>When it is given, click <b>Given</b> and enter the date, who gave it and the lot number.</li>
      <li>Use <b>Health record</b> for anything unusual: what you saw, how many animals, what you did and what a vet prescribed. A withdrawal period shows the date before which meat, eggs or milk should not be sold.</li>
    </ol>`,
  },
  {
    id: 'crops', title: 'Crop farming', who: 'Farm staff, Manager or Owner',
    html: `<ol>
      <li>Add each <b>plot</b> with its size in hectares, acres, plots or your own unit.</li>
      <li><b>Plant a crop</b>: crop, variety, plot, planting date, planting material and its cost, and when you expect to harvest.</li>
      <li><b>Record field work</b>: land preparation, weeding, fertiliser, spraying… with labour, other costs and any input taken from stock.</li>
      <li><b>Record harvest</b>: quantity, damaged, stored and sold. Stored produce can go straight into stock.</li>
    </ol>
    <p>FarmBuddy shows cost of production, yield per unit of land, harvest loss and money received. Produce in store is never counted as income. It does not give spray rates: follow the product label.</p>`,
  },
  {
    id: 'processing', title: 'Processing batches', who: 'Manager or Owner',
    html: `<p class="guide-lead">Palm oil, garri, flour, feed and other processing as batches with a real cost per unit.</p>
    <ol>
      <li><b>New batch</b>: what you are processing and the start date.</li>
      <li><b>Raw material</b>: for example 100 palm fruit bunches at ₦2,000. Or take it from stock.</li>
      <li><b>Costs</b>: milling fee (100 × ₦500), transport, labour, fuel, packaging.</li>
      <li>Tap the stages as the work moves along.</li>
      <li>When it is done, add each <b>output</b>: main products with their share of the cost (for example oil 85%, kernels 15%), by-products and waste. Link outputs to stock items.</li>
      <li><b>Close batch</b>: outputs go into stock at their cost per unit.</li>
    </ol>
    <p>Cost per unit = total batch cost × the product's share ÷ quantity produced. FarmBuddy never assumes a fixed yield.</p>`,
  },
  {
    id: 'palm-oil', title: 'Runs and sales: processing, harvests and selling', who: 'Manager or Owner (sales: anyone who sells)',
    html: `<p class="guide-lead">Each lot you process or harvest is a <b>run</b>. A run fills your barn. Each <b>sale</b> takes from it. When everything from a run is sold, its profit or loss is final. Many runs can be selling at the same time.</p>
    <h3>1. Start a processing run</h3>
    <ol>
      <li>Open <b>Processing</b> and tap <b>Start a processing run</b>. Choose palm oil, garri, fufu or cassava flour.</li>
      <li>Did you buy the cassava (or palm fruit), take it from your farm, or both? How do you buy it (bag, kg, basin…)? How much is one? How many did you buy?</li>
      <li>How much was your transport? Which mill did the processing, and how much was the fee?</li>
      <li>Workers, if any: each name and how much you paid. Any other expenses: each one and its price.</li>
      <li><b>How you sell it</b>: tick every way you sell in your area (bag, half bag, quarter bag, paint bucket, custard bucket, derica, cup, kg…) and put today's price. Choose the one you count your stock in (for example bag) and say how many of each make one (for example 120 cups = 1 bag). You only do this once; FarmBuddy remembers it next time. Add your own measure if it is not listed.</li>
      <li>How much did you get? Choose the measure and enter the quantity. Tap <b>Save run</b>.</li>
    </ol>
    <h3>2. Record a harvest</h3>
    <p>For cucumber, pepper, vegetables, melon (egusi), water yam (ete) and yam. Tap <b>Record a harvest</b>: what you harvested, when, where, transport, workers, other expenses, how you sell it and how much you harvested.</p>
    <h3>3. Record a sale</h3>
    <ol>
      <li>Tap <b>Record a sale</b>. Choose the run it came from (each shows what is left).</li>
      <li>How did you sell it (bag, cup, custard bucket…), how many, and the price of one.</li>
      <li>The buyer's name and phone, and how they paid. If they paid part or nothing, the rest is recorded as owed and shows under Buyers.</li>
      <li>Transportation and miscellaneous costs for this sale.</li>
      <li>Tap <b>Save sale and make receipt</b>. Print it or send it on WhatsApp.</li>
    </ol>
    <h3>Stock that balances</h3>
    <p>What a run gets goes into your barn (Farm stock) in the measure you count it in, at the run's cost for each one. A sale in cups or custard buckets takes out the right part of a bag. You cannot sell more than a run has left. Own cassava or fruit kept in stock is taken out when you use it. To undo a sale, void its receipt in Receipts: the quantity goes back to the run and the run opens again.</p>
    <p>FarmBuddy never assumes how much you will get, how big a measure is in your market, or what prices are. Every number is yours.</p>`,
  },
  {
    id: 'batches', title: 'Livestock: from the day the birds arrive to the day they are sold', who: 'Everyone (money: Manager and Owner)',
    html: `<p class="guide-lead">Open <b>Livestock</b>. Answer simple questions; FarmBuddy counts the birds, the feed, the money and the dates.</p>
    <h3>1. Start a new batch (manager)</h3>
    <p>Name, type and breed, house, who is responsible, how many birds, arrival date and age, weeks of rearing and selling age. The chicks: ordered, delivered, dead on arrival, how many you paid for, price, transport, supplier and what you paid. The feed plan: bags per 100 birds (your farm standard of 17 bags of 25 kg is filled in; change it if your plan is different), and starter, grower and finisher bags. The selling plan and budget. Tick the broiler timetable to add the day 1–56 vaccination and health calendar (your vet must confirm the dates and products).</p>
    <p>Example: 600 birds × 17 bags ÷ 100 = <b>102 bags</b> (2,550 kg). With the practical split of 30% starter (weeks 1–4) and 70% finisher (weeks 5–8): 31 bags of starter (775 kg, about 27.7 kg a day) and 71 bags of finisher (1,775 kg). You can change the split. This is a budget, not a fact: FarmBuddy compares it with the feed really used.</p>
    <h3>2. Today's record (every day, anyone)</h3>
    <p>Deaths and the cause, whether they were disposed of safely, sick birds kept apart, birds removed or added, which feed and how many kg the birds got (FarmBuddy shows the plan for today in kg and takes exactly that weight out of the store), feed wasted, water, what you noticed, problems and tomorrow's plan. Saving again for the same day changes it, and the change is logged.</p>
    <h3>3. Feed store (manager)</h3>
    <p><b>Buy feed</b>: which feed, how many bags, price per bag, supplier, what you paid, condition and receipt. For a new feed say the bag weight, the level to warn you at and how many bags a full store holds. Each feed shows bags left, bags used a day, days left, the date it will finish and how many bags to buy. Warnings: reducing (50%), restocking required (25%), urgent (at your minimum), finished, and a critical reminder when it will finish within 3 days. The owner can change these limits (Livestock → Alert limits).</p>
    <h3>4. Vaccines, medicine and vitamins</h3>
    <p>Record every dose: name, brand or strain, how it was given, birds, expiry, cost, who gave it, reactions and the next due date. For medicine, enter the withdrawal period from the label or the vet: FarmBuddy then refuses to sell the birds before it ends. It never guesses a withdrawal period. On the timetable, tap <b>Record</b> when an item is done or <b>Not needed</b>.</p>
    <h3>5. Weigh birds (weekly)</h3>
    <p>How many birds you weighed and their total weight. FarmBuddy shows the average, the gain, grams a day, FCR (feed ÷ weight) and the expected weight on selling day.</p>
    <h3>6. Money spent (manager)</h3>
    <p>Category, item, quantity, price, who bought, supplier, paid and receipt. The owner approves expenses entered by others. Feed is recorded in the feed store and chicks on the batch, so nothing is counted twice.</p>
    <h3>7. Sell birds</h3>
    <p>Sell now or reserve for a buyer. Birds, per bird or per kg, price, buyer and phone, how paid (part payment is recorded as owed), delivery and its cost. A sale makes a receipt you can print or send on WhatsApp. Reserved birds are confirmed later with <b>Confirm sale</b>.</p>
    <h3>The batch page</h3>
    <p>Birds alive, deaths and %, feed bags left and when they finish, what to buy, vaccination due, money spent, sold, owed to you, profit or loss so far and the expected result when all birds are sold, with alerts. <b>Report</b> prints it (or saves as PDF); daily records export to Excel (CSV). Only the owner can remove a record, with a reason that is kept in the activity log.</p>`,
  },
  {
    id: 'advisory', title: 'Suggestions, reminders and the library',
    html: `<ul>
      <li><b>How it works, without internet:</b> FarmBuddy has a rules engine built into the app. Each time you open Suggestions it reads your own records on this computer (batches, crops, processing runs, stock, sales) and checks them against 20 rules. Nothing is sent anywhere. The owner can switch rules off, change their priority and the number of days under <b>Suggestion rules</b>. Guides and schedules arrive in the app or in signed update packages, which are checked before they are imported.</li>
      <li><b>Suggestions</b> come from your own records, for example a batch with no record for two days, a processing run heading for a loss, vaccinations due, a crop with no field work for two weeks, or a processing batch with no outputs. Each one says why it appeared.</li>
      <li><b>Reminders</b>: add your own, or create one from a suggestion. Due reminders show on Home.</li>
      <li><b>Knowledge library</b>: guides that work offline, each with its source (NAERLS, NiMet, the Seeds Council, NAFDAC, IITA…) and review status. Built-in guides stay <b>Pending expert review</b> until the owner records who reviewed them.</li>
    </ul>`,
  },
  {
    id: 'labels', title: 'Printing barcode labels', who: 'Manager or Owner',
    html: `<p class="guide-lead">Print price labels with barcodes for shelves and for items that came without one.</p>
    <ol><li>Inventory → <b>Print labels</b>.</li><li>Enter how many labels you need for each item.</li><li>Choose A4 sheets (24 labels, the common 63.5 × 33.9 mm label paper) or a label printer roll (50 × 30 mm), then print.</li></ol>
    <p>Items without a barcode get a shop code starting with 20, and it is saved to the item, so scanning the label on Sell produce adds that item.</p>`,
  },
  {
    id: 'updates', title: 'Updates', who: 'Owner',
    html: `<p class="guide-lead">You do not need to look for updates. When a new version is out, a window shows it on screen after you sign in.</p>
    <ol><li><b>Windows and Mac:</b> the update downloads in the background. Click <b>Restart and update</b>, and the app reopens on the new version.</li>
    <li><b>Android:</b> tap <b>Update now</b>. The phone downloads it (you see the progress, and a notification), then Android shows its install screen: tap <b>Update</b>. The first time, Android may ask you to allow FarmBuddy to install updates; switch it on and go back to the app.</li></ol>
    <p>Tap <b>Later</b> to be asked again next time. <b>Settings → Updates → Check now</b> looks straight away. Your data stays in place, and only updates signed by your provider are accepted.</p>`,
  },
  {
    id: 'afresh', title: 'Start afresh (clear test data)', who: 'Owner',
    html: `<p class="guide-lead">Tried the app with the ready-made items or test sales? Clear them and set up your real stock. Only the Owner can do this.</p>
    <ol><li>Open <b>Settings → Start afresh</b>.</li>
    <li>Choose whether to <b>also delete sales and test records</b> (receipts, customers, expenses and shifts). Leave it off to keep your sales history.</li>
    <li>Choose to start with an <b>empty item list</b>, or to <b>load the ready-made items again</b>.</li>
    <li>Enter your password, type <b>RESET</b>, tap <b>Start afresh</b> and confirm.</li></ol>
    <p>All items, categories, stock counts, stock history, expiry batches, deliveries and suppliers are deleted. Your licence, this device, business details, receipt settings, payment methods, colours, email settings and staff accounts stay the same.</p>
    <p class="guide-note">A safety backup is made first. If you reset by mistake, restore that backup (see Backups and your data).</p>`,
  },
  {
    id: 'customers', title: 'Customers and money owed',
    html: `<ul>
      <li>Add customers on the <b>Customers</b> page or during a sale. Only a name is required.</li>
      <li>Tick <b>Only those who owe</b> to see debtors and the total owed.</li>
      <li>Click <b>Receipts</b> on a customer's row to see every sale to them and record payments.</li>
      <li>Managers and owners can edit or export the list. A customer who owes money cannot be deleted.</li>
    </ul>`,
  },
  {
    id: 'reports', title: 'Overview and reports',
    html: `<figure class="guide-shot"><img src="/guide/overview.png" alt="The Today page"><figcaption>Today: your farm at a glance, reminders and money this month.</figcaption></figure>
    <p><b>Overview</b> opens after sign-in. Cashiers see their own figures only.</p>
    <h3>Reports (Manager or Owner)</h3>
    <p>Choose Today, Yesterday, This week, This month, Last month, Last 90 days, or your own dates. You get sales, gross profit and margin, money received, money still owed, sales by day, money received by method, sales by category and by staff, and every item sold with its profit. <b>Export items</b> saves a spreadsheet file to Downloads.</p>
    <h3>Balancing the cash at closing</h3>
    <ol><li>Open Reports and choose <b>Today</b>.</li><li>Compare <b>Money received by method</b> with the cash in the drawer, your bank app and the POS terminal.</li></ol>`,
  },
  {
    id: 'settings', title: 'Making it your own', who: 'Owner',
    html: `<ul>
      <li><b>Business profile:</b> name, address, phone, email, currency symbol and logo.</li>
      <li><b>Receipts &amp; tax:</b> receipt title, number prefix, footer, paper size (58 mm, 80 mm or A4) and VAT, with a live preview.</li>
      <li><b>Selling rules &amp; wording:</b> allow cashiers to change prices, allow selling at zero stock, and rename Item, Items and Customer to your own words.</li>
      <li><b>Colours &amp; theme:</b> six ready-made colour pairs or your own two colours, plus light or dark mode. Preview before you save.</li>
      <li><b>Payment methods:</b> rename, reorder, turn off, or add OPay, Moniepoint, PalmPay, Cheque and others.</li>
      <li><b>Business type:</b> add another type's categories and starting items at any time. Nothing you have is removed.</li>
      <li><b>Security &amp; recovery:</b> recovery key, security questions and recent security activity.</li>
    </ul>`,
  },
  {
    id: 'email', title: 'Email setup', who: 'Owner',
    html: `<p class="guide-lead">Receipts are sent from your own email account. Set it up in <b>Settings → Email</b>. It needs internet only when sending.</p>
    <h3>Gmail</h3>
    <ol>
      <li>Turn on <b>2-Step Verification</b> in your Google Account.</li>
      <li>Open <b>myaccount.google.com/apppasswords</b>, create one named <code>FarmBuddy</code> and copy the 16 letters.</li>
      <li>In Settings → Email, choose <b>Gmail / Google Workspace</b>, enter your Gmail address and paste the 16 letters as the App password.</li>
      <li>Click <b>Save email settings</b>, then <b>Send test</b> to your own address.</li>
    </ol>
    ${table(['Provider', 'Password to use'], [
      ['Outlook / Microsoft 365', 'Mailbox password. Business accounts may need the admin to allow Authenticated SMTP.'],
      ['Yahoo Mail', 'A Yahoo app password'],
      ['Zoho Mail', 'Password or an application-specific password'],
      ['Your domain email', 'Mailbox password. Ask your web host for the server and port.'],
    ])}`,
  },
  {
    id: 'backup', title: 'Backups and your data', who: 'Owner',
    html: `<p class="guide-lead">All records are in one file on this computer. FarmBuddy now copies it <b>automatically once a day</b>.</p>
    <ul>
      <li>Backups go to <b>Documents → FarmBuddy Backups</b>. The newest 14 are kept.</li>
      <li>In <b>Settings → Backup</b> you can choose another folder (a flash drive, or a Google Drive or OneDrive folder on this computer so copies leave the machine), change how many to keep, or click <b>Back up now</b>.</li>
      <li>If an automatic backup fails, Settings → Backup says why.</li>
    </ul>
    <h3>Restoring</h3>
    <ol>
      <li>Go to <b>Settings → Backup → Restore a backup</b> and choose a backup (or <b>Choose another file…</b> for one on a flash drive).</li>
      <li>FarmBuddy checks the file first and shows what is in it. A damaged file, a file from a newer version, or one with no owner account is refused.</li>
      <li>Enter your password and type <b>RESTORE</b>. FarmBuddy first saves a safety copy of the data on this computer, then restores and upgrades the backup if it came from an older version.</li>
      <li>Everyone signs in again with an account from the backup.</li>
    </ol>
    <p>If the app will not open at all, the manual way still works: close it, then replace <code>farmbuddy.sqlite3</code> in the data folder (Windows <code>%APPDATA%\\online.techalphahub.farmbuddy</code>, Mac <code>~/Library/Application Support/online.techalphahub.farmbuddy</code>) with the backup renamed to that name.</p>
    <p>A restored backup also brings back the recovery key and security questions that were set when it was made. The licence is tied to the computer, not the backup.</p>`,
  },
  {
    id: 'android', title: 'Using FarmBuddy on Android',
    html: `<p class="guide-lead">FarmBuddy also runs on Android phones and tablets, with the same features and the same licence prices.</p>
    <ol>
      <li><b>Install:</b> download FarmBuddy for Android from techalphahub.online/products/farmbuddy/download, open the file and tap <b>Install</b>. Works on Android 7.0 and newer.</li>
      <li><b>Moving around:</b> the bar at the bottom goes to Home, Sell, Receipts, Customers and Menu. The phone's back button works too.</li>
      <li><b>Scan barcodes:</b> on New sale, tap <b>Scan</b> and point the camera at the barcode. Allow camera access the first time.</li>
      <li><b>Print receipts:</b> install the free <b>RawBT Print Service</b> app from the Play Store and pair your Bluetooth or USB receipt printer in it once. Then tap <b>Print receipt</b> in FarmBuddy.</li>
      <li><b>Updates:</b> a new version shows on screen by itself. Tap <b>Update now</b>, then <b>Update</b> on Android's screen. Your data stays.</li>
    </ol>
    <p>Each phone needs its own licence key, like each computer. Labels, statements and shift reports print from the Windows or Mac app; statements can be sent on WhatsApp from the phone.</p>`,
  },
  {
    id: 'shortcuts', title: 'Shortcuts and tips',
    html: `${table(['Key', 'What it does'], [['<kbd>F2</kbd>', 'Open New sale, or jump to its search box'], ['<kbd>Enter</kbd>', 'Add the scanned or typed item'], ['<kbd>F9</kbd>', 'Take payment'], ['<kbd>Esc</kbd>', 'Close a pop-up window']])}
    <ul>
      <li><b>Produce store or input shop:</b> save each product's barcode once, or print labels for those without one. Set bag or crate prices for items you sell in bulk.</li>
      <li><b>Poultry:</b> record deaths and feed every day; FCR and cost per bird stay correct only with daily records.</li>
      <li><b>Livestock farm:</b> keep a daily record on the Livestock page; it works out mortality, FCR and profit for you.</li>
      <li><b>Farm produce:</b> sell to regular buyers on credit, and check Customers → Only those who owe each week.</li>
    </ul>`,
  },
  {
    id: 'privacy', title: 'What the app sends to TechAlpha',
    html: `<p class="guide-lead">FarmBuddy works offline. When this device has internet, it sends TechAlpha one small notice the first time, and at most one a day after that.</p>
    <p>This tells TechAlpha how many businesses use the app and which versions are in use, so we know who to tell about updates.</p>
    ${table(['Sent', 'Never sent'], [[
      'The Install ID, app version and device type; the business name and type from Settings; whether the app is licensed; the number of items, receipts, customers and staff accounts',
      'Sales details, prices or amounts; customer names, phone numbers or emails; passwords; email settings; your database or backups',
    ]])}
    <p>If there is no internet, nothing is sent and nothing slows down. The app simply tries again later.</p>`,
  },
  {
    id: 'troubleshooting', title: 'Troubleshooting',
    html: table(['Problem', 'What to do'], [
      ['Forgot my password', 'Click <b>Forgot password?</b> on the sign-in screen, or ask the owner to reset it under Staff.'],
      ['Lost the recovery key', 'Use the security questions instead. Then, once signed in, create a new key under Settings → Security &amp; recovery.'],
      ['"Too many failed attempts"', 'Wait 1 minute (sign-in) or 10 minutes (recovery) and try again.'],
      ['"This account has been deactivated"', 'The owner ticks Account active under Staff → Edit.'],
      ['I cannot see Inventory or Reports', 'Your account is a Cashier. Ask the owner.'],
      ['An item is greyed out on Sell produce', 'It is out of stock. Receive stock in Inventory.'],
      ['The scanner types the code but nothing is added', 'No item has that barcode yet. A manager can add it from Sell produce, or in Inventory.'],
      ['"This sale is not fully paid"', 'Add a customer to the sale, or take the full amount.'],
      ['The receipt prints too wide or small', 'Match the paper size in Settings → Receipts &amp; tax to your roll, and choose the same size in the print dialog with margins set to None.'],
      ['Gmail did not accept the sign-in', 'Use an App Password, not your Gmail password.'],
      ['"Could not reach the server" when emailing', 'Check the internet connection and the server and port in Settings → Email.'],
      ['"Open your shift before selling"', 'Click Open shift on New sale and count the float. The owner can switch this rule off.'],
      ['The activation screen appeared', 'The trial or licence has ended. Send the Install ID shown to your provider and enter the key they send. Nothing has been deleted.'],
      ['"This licence key was made for a different computer"', 'Each computer needs its own key. Send this computer\'s Install ID to your provider.'],
      ['WhatsApp did not open', 'Install WhatsApp Desktop, or sign in to web.whatsapp.com in your browser, and try again.'],
      ['A cashier cannot return items', 'Returns need a Manager or the Owner.'],
    ]),
  },
];
