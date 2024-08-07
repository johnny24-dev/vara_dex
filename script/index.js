const {GearApi, GearKeyring} = require('@gear-js/api')
const fs = require('fs');

const code = fs.readFileSync(`/Users/namng/Desktop/vara_contracts/dapps/contracts/target/wasm32-unknown-unknown/debug/dex_factory.opt.wasm`);

const node_info = async () => {
    const gearApi = await GearApi.create({providerAddress:'wss://testnet.vara.network'});
    const [chain, nodeName, nodeVersion] = await Promise.all([
        gearApi.chain(),
        gearApi.nodeName(),
        gearApi.nodeVersion(),
      ]);

      console.log(
        `You are connected to chain ${chain} using ${nodeName} v${nodeVersion}`,
      );
}

const create_account_from_seed = async (seed, name) => {
    const keyring = await GearKeyring.fromSeed(seed, name);
    return keyring
}

const create_account_from_mnemonic = async (mnemonic, name) => {
    const keyring = GearKeyring.fromMnemonic(mnemonic, name);
    return keyring
}

const create_program = async (code,init_payload) => {

    const program = {
        code,
        gasLimit: 1000000,
        value: 1000,
        initPayload: payload,
      };
      
    try {
        const gearApi = await GearApi.create({providerAddress:'wss://testnet.vara.network'});
        const {programId, codeId, salt, extrinsic} = gearApi.program.create(program);
        const account = await create_account_from_mnemonic(`glimpse code swing mind owner three blossom later submit violin coin brass`,'Anme')
        
        console.log("🚀 ~ create_program= ~ programId", programId)
        console.log("🚀 ~ create_program= ~ codeId", codeId)
        console.log("🚀 ~ create_program= ~ salt", salt)
        console.log("🚀 ~ create_program= ~ extrinsic", extrinsic)
        
        await extrinsic.signAndSend(account, (event) => {
            console.log(event.toHuman());
          });


       

    } catch (error) {
        console.log("🚀 ~ constcreate_program= ~ error:", error)
    }
}

