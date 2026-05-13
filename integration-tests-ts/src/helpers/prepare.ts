import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {Worker, type NearAccount} from 'near-workspaces';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const RES_DIR = path.resolve(__dirname, '..', '..', '..', 'res');

const WASM = {
  sweat: path.join(RES_DIR, 'sweat.wasm'),
  claim: path.join(RES_DIR, 'sweat_claim.wasm'),
  stub: path.join(RES_DIR, 'exploit_stub.wasm'),
} as const;

const FT_POSTFIX = '.u.sweat.testnet';
const LONG_ACCOUNT_NAME = 'a'.repeat(30);

export interface Accounts {
  readonly root: NearAccount;
  readonly sweat: NearAccount;
  readonly claim: NearAccount;
  readonly stub: NearAccount;
  readonly oracle: NearAccount;
  readonly alice: NearAccount;
  readonly bob: NearAccount;
  readonly long: NearAccount;
}

export interface WorkspaceContext {
  readonly worker: Worker;
  readonly accounts: Accounts;
}

interface Contracts {
  readonly sweat: NearAccount;
  readonly claim: NearAccount;
  readonly stub: NearAccount;
}

interface Users {
  readonly oracle: NearAccount;
  readonly alice: NearAccount;
  readonly bob: NearAccount;
  readonly long: NearAccount;
}

interface StorageBalanceBounds {
  readonly min: string;
  readonly max: string | null;
}

export async function initWorker(): Promise<WorkspaceContext> {
  const worker = await Worker.init();
  const root = worker.rootAccount;

  const contracts = await deployContracts(root);
  const users = await createUserAccounts(root);

  await initSweat(contracts.sweat, users.oracle);
  await initStub(contracts.stub);
  await initClaim(contracts.claim, contracts.sweat.accountId);

  await registerForFtStorage(contracts.sweat, [
    users.oracle,
    users.alice,
    users.long,
    contracts.claim,
  ]);

  return {
    worker,
    accounts: {root, ...contracts, ...users},
  };
}

async function deployContracts(root: NearAccount): Promise<Contracts> {
  const sweat = await root.devDeploy(WASM.sweat);
  const claim = await root.devDeploy(WASM.claim);
  const stub = await root.devDeploy(WASM.stub);
  return {sweat, claim, stub};
}

async function createUserAccounts(root: NearAccount): Promise<Users> {
  const oracle = await root.createSubAccount('oracle');
  const alice = await root.createSubAccount('alice');
  const bob = await root.createSubAccount('bob');
  const long = await root.createSubAccount(LONG_ACCOUNT_NAME);
  return {oracle, alice, bob, long};
}

async function initSweat(sweat: NearAccount, oracle: NearAccount): Promise<void> {
  await sweat.call(sweat, 'new', {postfix: FT_POSTFIX});
  await sweat.call(sweat, 'add_oracle', {account_id: oracle.accountId});
}

async function initStub(stub: NearAccount): Promise<void> {
  await stub.call(stub, 'new', {});
}

async function initClaim(claim: NearAccount, tokenAccountId: string): Promise<void> {
  await claim.call(claim, 'init', {token_account_id: tokenAccountId});
  await claim.call(claim, 'add_oracle', {account_id: tokenAccountId});
}

async function registerForFtStorage(
  ft: NearAccount,
  accounts: readonly NearAccount[],
): Promise<void> {
  const attachedDeposit = await ftStorageMin(ft);
  await Promise.all(
    accounts.map(account =>
      account.call(
        ft,
        'storage_deposit',
        {account_id: account.accountId},
        {attachedDeposit},
      ),
    ),
  );
}

async function ftStorageMin(ft: NearAccount): Promise<bigint> {
  const bounds = await ft.view<StorageBalanceBounds>('storage_balance_bounds');
  return BigInt(bounds.min);
}
