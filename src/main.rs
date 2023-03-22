#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{sync::mpsc::{channel, Sender, Receiver}, thread::{spawn, JoinHandle}, time::Duration};

use anyhow::Result;
use opencv::{videoio::{VideoCapture, self, CAP_PROP_FRAME_WIDTH, CAP_PROP_FRAME_HEIGHT, VideoCaptureTrait}, prelude::{Mat, MatTraitConstManual}, imgproc::{cvt_color, COLOR_BGR2RGBA, get_rotation_matrix_2d, warp_affine, INTER_LINEAR}, core::{Point2f, Scalar, BORDER_CONSTANT}};
use slint::{Timer, TimerMode, Image};
slint::include_modules!();
const CANVAS_WIDTH: u32 = 640;
const CANVAS_HEIGHT: u32 = 480;
const FPS:f32 = 30.0;

const CAMERA_INDEX:i32 = 1;

fn main() -> Result<()>{
    let window = Main::new();
  
    let timer = Timer::default();
    let window_clone = window.as_weak();

    let (frame_sender, frame_receiver) = channel();
    let (exit_sender, exit_receiver) = channel();

    let mut frame_data = vec![0; (CANVAS_WIDTH * CANVAS_HEIGHT * 4) as usize];

    // 30帧速度刷新
    timer.start(TimerMode::Repeated, std::time::Duration::from_secs_f32(1./FPS), move || {
        if let Some(window) = window_clone.upgrade(){
            window.set_frame(window.get_frame()+1);
        }
    });

    let task = start(frame_sender, exit_receiver);

    let mut render = move || -> Result<Image>{

        if let Ok(frame_rgba_rotated) = frame_receiver.try_recv(){
            frame_data.copy_from_slice(&frame_rgba_rotated);
        }

        let v = slint::Image::from_rgba8(slint::SharedPixelBuffer::clone_from_slice(
            frame_data.as_slice(),
            CANVAS_WIDTH,
            CANVAS_HEIGHT,
        ));
        Ok(v)
    };

    window.on_render_image(move |_frame|{
        render().map_err(|err| eprintln!("{:?}", err)).unwrap()
    });

    window.run();
    println!("窗口关闭..");
    exit_sender.send(())?;
    let result = task.join();
    println!("程序结束{:?}", result);
    Ok(())
}

/// 启动拍照线程
fn start(frame_sender: Sender<Vec<u8>>, exit_receiver: Receiver<()>) -> JoinHandle<Result<()>>{
    spawn(move || -> Result<()>{
        //打开相机
        let mut camera = VideoCapture::new(CAMERA_INDEX, videoio::CAP_DSHOW)?;
        camera.set(CAP_PROP_FRAME_WIDTH, CANVAS_WIDTH as f64)?;
        camera.set(CAP_PROP_FRAME_HEIGHT, CANVAS_HEIGHT as f64)?;
        // camera.set(CAP_PROP_FPS, 30.)?;

        // let fourcc = VideoWriter::fourcc('M','J','P','G')?;
        // camera.set(CAP_PROP_FOURCC,fourcc as f64)?;

        // let fourcc = VideoWriter::fourcc('Y','U','1','2')?;
        // camera.set(CAP_PROP_FOURCC,fourcc as f64)?;

        let mut frame_bgr = Mat::default();
        let mut frame_rgba = Mat::default();
        let mut frame_rgba_rotated = Mat::default();
        let mut angle = 0.0;
        
        loop{
            if let Ok(()) = exit_receiver.try_recv(){
                break;
            }
            camera.read(&mut frame_bgr)?;

            //bgr 转 rgba
            cvt_color(&frame_bgr, &mut frame_rgba, COLOR_BGR2RGBA, 0)?;
            
            // 旋转图像
            angle += 3.;
            if angle > 360. {
                angle = 0.;
            }
            let mat = get_rotation_matrix_2d(Point2f::new(CANVAS_WIDTH as f32/2., CANVAS_HEIGHT as f32/2.), angle, 1.)?;
            warp_affine(&frame_rgba, &mut frame_rgba_rotated, &mat, opencv::core::Size::new(CANVAS_WIDTH as i32, CANVAS_HEIGHT as i32), INTER_LINEAR, BORDER_CONSTANT, Scalar::default())?;

            frame_sender.send(frame_rgba_rotated.data_bytes()?.to_vec())?;

            std::thread::sleep(Duration::from_millis(10));
        }
        println!("拍照线程结束..");
        Ok(())
    })
}