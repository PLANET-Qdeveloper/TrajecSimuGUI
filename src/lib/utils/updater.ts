import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { ask, message } from '@tauri-apps/plugin-dialog';

export async function checkForUpdates() {
  try {
    const update = await check();
    
    // アップデートがない場合はここで終了
    if (!update) {
      console.log('no updates available');
      return;
    }

    // 1. ユーザーにアップデートするか確認するダイアログを表示
    const userAgreed = await ask(
      `新しいバージョン (${update.version}) が利用可能です。\n\n今すぐダウンロードして再起動しますか？`,
      {
        title: 'アップデートの確認',
        kind: 'info',
        okLabel: 'アップデートする',
        cancelLabel: 'あとで'
      }
    );

    // ユーザーが「あとで」を選んだ場合は処理をキャンセル
    if (!userAgreed) {
      console.log('アップデートがキャンセルされました。');
      return;
    }

    let downloaded = 0;
    let contentLength: number | undefined = 0;
    
    await update.downloadAndInstall((event) => {
      switch (event.event) {
        case 'Started':
          contentLength = event.data.contentLength;
          console.log(`ダウンロード開始: ${event.data.contentLength} bytes`);
          break;
        case 'Progress':
          downloaded += event.data.chunkLength;
          console.log(`ダウンロード中: ${downloaded} / ${contentLength}`);
          // ※ ここでSvelteのストア(store)に進行状況を渡せば、画面上にプログレスバーを出すことも可能です
          break;
        case 'Finished':
          console.log('ダウンロード完了');
          break;
      }
    });

    // 3. 完了したら再起動する旨を伝えてから再起動
    await message('アップデートが完了しました。アプリを再起動します。', { 
      title: '再起動', 
      kind: 'info',
      okLabel: 'OK'
    });
    
    await relaunch();

  } catch (error) {
    console.error('アップデート処理中にエラーが発生しました:', error);
    await message('アップデートの確認中にエラーが発生しました。', { 
      title: 'エラー', 
      kind: 'error' 
    });
  }
}